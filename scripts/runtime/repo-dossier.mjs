import { runGraphCli } from './worker-lock.mjs'
// repo-dossier — per-repo, evidence-attached memory mined from the experience
// ledger and native authored-write verdicts.
//
// The cockpit's D1 events record what actually ran in each workspace and
// whether it worked (`kind:"event"`, subkind `cmd`: scrubbed text, exit status,
// duration, repo stamp). This module mines those into per-repo FACTS — rituals
// ("`cargo test` is this repo's test command and passes"), traps ("`npm test`
// always fails here"), and the last-session thread — and folds them into the
// shared causal graph as HYPOTHESIS nodes, the third instance of the
// self-causal / config-causal pattern:
//   • mining attaches OBSERVATIONAL stats only (ride on the node, no signed
//     edges) — observation prioritizes, verification concludes;
//   • idle probes (D4) add SUPPORTS/CONTRADICTS edges via updateBeliefs;
//   • compile blends both into a per-fact belief, decays it toward 0.5 with
//     age since last verification, and emits the injection artifact the
//     cockpit renders at session start (D3): ~/.angel0/dossier/<key>.json.
//
// Pure core (no file I/O, no network) + a CLI shell at the bottom, exactly the
// config-causal.mjs layout. Node ids embed the repo key VERBATIM — the key is
// minted Rust-side (work_landing::workspace_key) and never re-derived here.

import { storeCap } from './store-caps.mjs'
import { NODE_TYPE } from '../../lib/research/CausalGraph.js'
import { beliefProbability, scoreHypotheses } from './causal-loop.mjs'
import { isMachineLabeled } from './cut-evidence.mjs'
import { join } from 'node:path'
import { homedir } from 'node:os'

export function dossierPaths(argv = [], env = process.env, userHomeDir = homedir()) {
  const flag = (name) => {
    const index = argv.indexOf(name)
    return index >= 0 ? argv[index + 1] : undefined
  }
  const configured = (value) => (typeof value === 'string' && value.trim() ? value : undefined)
  const stateDir = join(userHomeDir, '.angel0')
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

export function assertRepoKey(key) {
  if (typeof key !== 'string' || !/^[A-Za-z0-9][A-Za-z0-9_-]{0,191}$/u.test(key)) {
    throw new Error('Invalid dossier repository key')
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

export const DOSSIER_PROJECT = Object.freeze({ id: 'repo-dossier', label: 'Repo dossier' })

// ─── fact-minting thresholds ─────────────────────────────────────────────────
// A ritual needs recurrence across sessions before it's believed to be "how
// this repo works" — one-off commands never become facts.
export const RITUAL_MIN_RUNS = 3
export const RITUAL_MIN_SESSIONS = 2
// A trap is a command that keeps failing: enough attempts to matter, almost
// never passing.
export const TRAP_MIN_FAILS = 3
export const TRAP_MAX_PASS_RATE = 0.2
// Observational (mining-only) evidence can shift belief at most this far from
// 0.5 — the same "prioritize, don't conclude" cap Reflex uses (≤0.4).
export const OBSERVATIONAL_WEIGHT = 0.4
// Unverified beliefs drift back toward 0.5 with this half-life, so a stale
// fact drops below the injection threshold and becomes a probe target instead
// of a stale assertion.
export const DEFAULT_HALF_LIFE_DAYS = 14

const slug = (s) =>
  String(s ?? '')
    .toLowerCase()
    .replace(/[^0-9a-z]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48) || 'x'

const isDossier = (node) => node?.projectId === DOSSIER_PROJECT.id

// ─── command normalization ───────────────────────────────────────────────────
// Agents do not type `cargo test`. They type
//
//     cd ./worktree/cockpit && cargo check 2>&1 | tail -20
//
// and every part of that shape defeats a naive miner:
//
//   • the leading `cd …` chain hides the command from a `^`-anchored classifier
//     (measured on the live ledger: 9,736 of 9,784 recorded commands were
//     unclassifiable, so no repo could mint a ritual no matter how much it ran);
//   • the chdir target is often an EPHEMERAL worktree, so keeping it in the
//     canonical text would mint a "ritual" pointing at a directory that no
//     longer exists — and `dossier::build_ritual` hands that text to The Cut as
//     a verify command;
//   • legacy rows let trailing `| tail -20` own the exit status. Current v3
//     rows carry `verdict`, `pipefail`, and provenance, but the canonical fact
//     is still the first stage rather than its display plumbing.
//
// So: strip the chdir and keep the FIRST stage as the command. Normalization
// marks later stages so the mining layer can combine that shape with the row's
// schema/provenance before deciding whether a verdict is attributable.

// A leading `cd <target> && | ; ` chain (repeatable). The target may be quoted
// or a `$(…)` substitution.
const CHDIR_RE = /^\s*cd\s+("[^"]*"|'[^']*'|(?:\$\([^)]*\)|[^\s;&|])+)\s*(?:&&|;)\s*/
// Where one stage of a shell line ends: an unquoted pipe / chain / separator.
const STAGE_END_RE = /\s*(\|\||&&|[|;])\s*/
// Trailing plumbing that is not part of the command's identity.
const REDIR_RE = /\s*(?:\d?>>?\s*\S+|\d?>&\d?|<\s*\S+)\s*$/

const unquote = (s) => s.replace(/^["']|["']$/g, '')

/** Is `target` (a `cd` argument) inside the repo the row was stamped with? */
function chdirStaysInRepo(target, { root, cwd } = {}) {
  const t = unquote(String(target ?? '').trim())
  // Relative, `$(pwd)`, `~`, `-`: cannot escape the recorded cwd in any way we
  // can resolve, and by construction the recorded cwd is in-repo.
  if (!t.startsWith('/')) return true
  const inside = (base) =>
    base && (t === base || t.startsWith(base.endsWith('/') ? base : base + '/'))
  return Boolean(inside(root) || inside(cwd))
}

/**
 * Reduce a recorded command line to the command whose exit status was observed.
 * Pure.
 *
 * @param {string} text  the recorded command line
 * @param {{root?:string, cwd?:string}} [repo]  the row's repo stamp, used to
 *   reject a `cd` into a DIFFERENT repository — `cd ~/angel0 && cargo test` run
 *   from inside gpug is evidence about angel0, and must never become a gpug fact.
 * @returns {{command:string, attributable:boolean}|null}  null when the sample
 *   belongs to another repo. `attributable` is false when the recorded exit
 *   status is some later stage's, not this command's.
 */
export function normalizeCommand(text, repo = {}) {
  let t = String(text ?? '').trim()
  let m
  while ((m = CHDIR_RE.exec(t))) {
    if (!chdirStaysInRepo(m[1], repo)) return null // another repo's evidence
    t = t.slice(m[0].length)
  }
  const end = t.search(STAGE_END_RE)
  const attributable = end < 0
  if (!attributable) t = t.slice(0, end)
  return { command: t.replace(REDIR_RE, '').trim(), attributable }
}

// ─── command classification ──────────────────────────────────────────────────
// Head-of-command rules: the NORMALIZED command's leading tokens decide the
// class. Only classified commands can become rituals; anything can become a trap.
const CLASS_RULES = [
  [
    'test',
    /^(cargo (test|nextest)|npm (run )?test|npx (vitest|jest)|pytest|go test|node --test|make test)\b/,
  ],
  ['build', /^(cargo build|npm run build|npx tsc|tsc|go build|make(\s|$)|cargo check)\b/],
  [
    'lint',
    /^(cargo (clippy|fmt)|npm run (lint|format)|npx (eslint|prettier)|ruff|eslint|prettier)\b/,
  ],
  ['run', /^(cargo run|npm (run )?(dev|start)|npx vite|python3? \S)\b/],
]

/** Classify a recorded command into a ritual class, or null when unclassified. */
export function classifyCommand(text) {
  const t = String(text ?? '').trim()
  for (const [cls, re] of CLASS_RULES) if (re.test(t)) return cls
  return null
}

// ─── The Cut: the second verdict source ──────────────────────────────────────
// The ledger's `cmd` events are what an AGENT TYPED, and it types pipelines:
// `cargo check 2>&1 | tail -20` records tail(1)'s exit, so the row proves
// recurrence and nothing else. On the live ledger that is not an edge case — it
// is every single build command angel0 has ever recorded, which is why this
// repo's dossier has minted exactly zero facts since the day it shipped.
//
// The native Cut verifier does not have that problem. It runs its
// verify DIRECTLY — no shell, no pipe — and stamps the true exit code onto the
// authored-write manifest (`~/.angel0/cut/authored-*.jsonl`, `cockpit/src/knowledge/cut.rs`).
// Every one of those rows is an attributable verdict. So the manifest is mined
// here as a first-class verdict source alongside the ledger.
//
// ─── THE FEEDBACK LOOP, AND THE RULE THAT BREAKS IT ──────────────────────────
//
// This closes a circle, and the circle has to be cut deliberately or the whole
// exercise manufactures confident nonsense.
//
// The Cut picks its verify command with the precedence (`cut.rs::resolve_verify`)
//
//     ANGEL_CUT_VERIFY (env)  →  the dossier's learned build ritual  →  default
//
// The middle arm is the problem. If the dossier learns "`cargo check` is this
// repo's build ritual" from verdicts produced by runs that chose `cargo check`
// BECAUSE THE DOSSIER SAID SO, then it is counting its own recommendation as
// evidence for that recommendation. Belief climbs with zero new information, the
// injection gate is cleared by a fact that is true only of itself, and the
// dossier ends up asserting — with rising confidence — whatever it happened to
// say first. That is the exact failure this branch exists to prevent.
//
// The manifest records the provenance we need: `machine.source` is `env`,
// `dossier`, or `default` (the `source` field on `cut.rs::VerifyPlan`). So:
//
//   **A dossier-sourced verdict may never SUPPORT the fact that selected it.
//     It may only CONTRADICT.**
//
//   • `source: "default"` / `"env"` — the command was chosen INDEPENDENTLY of
//     what the dossier believes. Full vote: it establishes the ritual, it counts
//     toward recurrence, and it may raise or lower belief.
//   • `source: "dossier"` — the dossier told The Cut to run this. "I did what you
//     told me and it worked" is not independent confirmation that it was the
//     right thing to do. It may only CONTRADICT: a FAILURE counts (a learned
//     ritual that starts failing is real news, and belief must fall), a PASS is
//     inert.
//   • `source` absent/unknown — treated as dossier-sourced, i.e. contradict-only.
//     Fail-closed: an unprovenanced verdict can never inflate a belief.
//
// Mechanically, "may only contradict" means a dossier-sourced sample touches
// NOTHING that can move belief upward:
//
//   • not `runs`/`sessions` — recurrence is what mints a ritual into existence
//     (RITUAL_MIN_RUNS/RITUAL_MIN_SESSIONS), so a self-selected run must not vote
//     itself into being;
//   • not the canonical-form election — the dossier gets no vote on what its own
//     ritual is;
//   • not `passes`, and not the verdict denominator when it passed;
//   • **not `lastSeen`** — and this one is subtle enough to be worth the ink. The
//     brief for this change said a dossier-sourced verdict "may refresh
//     lastVerifiedAt". It may not, and the reason is [`factBelief`]'s decay term:
//     `belief = 0.5 + (raw - 0.5) * 0.5^(age/halfLife)`. Refreshing the clock
//     drives `decay` toward 1, which pushes a >0.5 belief UP. A self-selected
//     pass that keeps the clock warm keeps its own fact permanently fresh and
//     permanently believed — the same feedback loop, laundered through the
//     freshness term instead of the pass count. Freshness is a belief-raising
//     channel, so it is support in disguise, so dossier-sourced rows do not get
//     it.
//
// A dossier-sourced FAILURE does count: it joins the verdict denominator and the
// fail count. That strictly LOWERS belief (the pass-rate drop always dominates
// the saturation bump — see the test), and it leaves the decay clock alone, so
// the fall is monotone and cannot be undone by a freshness bonus.
//
// The safety property that falls out, and the reason this is not merely a
// hair-shirt: a learned ritual whose only remaining evidence is self-selected
// decays back toward 0.5 (nothing refreshes its clock), drops under the 0.70
// injection gate, stops being handed to The Cut — at which point `resolve_verify`
// falls through to `default`, INDEPENDENT verdicts start flowing again, and the
// fact is re-earned or not. The circle is not just cut; it is replaced by a
// periodic demand for independent re-confirmation.

/** Verify-command sources that were chosen independently of the dossier's belief. */
export const CUT_INDEPENDENT_SOURCES = Object.freeze(['default', 'env'])

/**
 * One Cut manifest row → a verdict sample, or null when the row is not a verdict.
 *
 * Not verdicts, and they take no part in anything (`isMachineLabeled`, shared
 * with cut-audit so there is one definition of "the machine adjudicated this"):
 *   • `machine: {skipped: "not-source"}` — a docs edit does not earn a compile;
 *   • `machine: {timed_out: true}` — "we ran out of time" is not "you broke it";
 *   • no `machine` at all — the write was never checked.
 *
 * Pure.
 */
export function cutSample(row) {
  if (row?.kind !== 'authored' || !row?.repo?.key) return null
  if (!isMachineLabeled(row)) return null
  const m = row.machine
  const text = String(m.cmd ?? '').trim()
  if (!text) return null
  return {
    text,
    // Unpiped and run directly by cut.rs, so this exit code is the command's own.
    // That is the entire reason this source exists.
    ok: Number(m.exit) === 0,
    ts: Number(row.ts) || 0,
    session: row.session ?? 0,
    dur: Number(m.dur_ms) || 0,
    independent: CUT_INDEPENDENT_SOURCES.includes(String(m.source ?? '')),
  }
}

// ─── mining ──────────────────────────────────────────────────────────────────

const isCmdEvent = (r) => r?.kind === 'event' && r?.event === 'cmd' && r?.repo?.key && r?.cmd?.text
const isTurn = (r) => (r?.kind === 'turn' || r?.kind === 'moa_turn') && r?.repo?.key

/**
 * Read the verdict carried by a v3 command row.
 *
 * The old ledger exposed only `exit`, so normalization had to reject every
 * pipeline: under `sh`, that status belonged to the last stage.  The current
 * writer records an explicit verdict plus the shell/pipefail provenance.  A
 * successful pure pipeline under pipefail proves its first stage succeeded;
 * a failed pipeline does not identify which stage failed, so it remains
 * unattributable to the canonical first-stage command.  `;` and `||` can also
 * hide an earlier failure and are therefore never promoted to a verdict.
 *
 * Legacy rows retain the old attribution rule.  Unknown future verdict values
 * fail closed.
 */
function ledgerOutcome(row, norm) {
  const cmd = row.cmd || {}
  if (typeof cmd.verdict !== 'string') {
    return norm.attributable ? cmd.exit === 0 && !cmd.timed_out : null
  }
  if (cmd.verdict === 'no_verdict') return null
  if (cmd.verdict !== 'pass' && cmd.verdict !== 'fail') return null
  if (norm.attributable) return cmd.verdict === 'pass'

  // Only a successful conjunction/pipeline can prove its first stage. A
  // failure says merely that *some* stage failed. A sequence or recovery arm
  // (`;`, `||`) cannot prove even that much about the first stage.
  const text = String(cmd.text ?? '')
  if (
    cmd.verdict === 'pass' &&
    cmd.pipefail === true &&
    !text.includes(';') &&
    !text.includes('||')
  ) {
    return true
  }
  return null
}

/** Whether this command choice was independent of the dossier's own prompt. */
function ledgerIndependent(cmd) {
  if (typeof cmd?.independent === 'boolean') return cmd.independent
  if (typeof cmd?.source === 'string') return cmd.source !== 'dossier'
  // Legacy rows predate prompt-provenance tracking. Preserve their historical
  // behavior; their piped exits are still rejected by ledgerOutcome above.
  return true
}

/**
 * Mine parsed ledger rows and Cut manifest rows into per-repo fact candidates.
 * Pure.
 *
 * @param {object[]} rows     parsed ledger records (any kinds; only cmd events
 *                            and turn/moa_turn rows are read)
 * @param {object[]} [cutRows]  parsed Cut manifest rows (`kind:"authored"`), the
 *   second verdict source. Unlike the ledger's shell pipelines these carry a
 *   REAL exit code — and a provenance (`machine.source`) that decides whether the
 *   verdict is allowed to vote for the command it ran. See CUT_INDEPENDENT_SOURCES.
 * @returns {Object<string, {root:string|null, slug:string|null,
 *   rituals:object[], traps:object[], thread:object|null}>}  keyed by repo.key
 */
export function mineRepoFacts(rows, cutRows = []) {
  const repos = {}
  const repo = (r) =>
    (repos[r.repo.key] ||= {
      root: r.repo.root ?? null,
      slug: r.repo.slug ?? null,
      byClass: {},
      byText: {},
      rituals: [],
      traps: [],
      thread: null,
    })
  const push = (rec, sample) => {
    const cls = classifyCommand(sample.text)
    if (cls) (rec.byClass[cls] ||= []).push(sample)
    ;(rec.byText[sample.text] ||= []).push(sample)
  }

  for (const r of rows || []) {
    if (isCmdEvent(r)) {
      const rec = repo(r)
      const norm = normalizeCommand(r.cmd.text, r.repo)
      if (!norm?.command) continue // another repo's work, or nothing left
      // `ok` is a TRISTATE: true / false / null. Current rows carry an explicit
      // verdict and the provenance needed to read it; legacy rows fall back to
      // the old unpiped-only rule. Null samples still prove recurrence but never
      // vote on pass or fail.
      const ok = ledgerOutcome(r, norm)
      push(rec, {
        text: norm.command,
        ok,
        ts: Number(r.ts) || 0,
        session: r.session ?? 0,
        dur: Number(r.cmd.dur_ms) || 0,
        // Current writers observe whether the dossier itself asserted this
        // command into the prompt. Its echo may contradict, never support.
        independent: ledgerIndependent(r.cmd),
      })
    } else if (isTurn(r)) {
      const rec = repo(r)
      const t = {
        ts: Number(r.ts) || 0,
        stop: r.outcome?.stop ?? r.outcome?.route ?? null,
        driver: r.driver ?? null,
        ok: r.outcome?.ok ?? null,
      }
      if (!rec.thread || t.ts > rec.thread.ts) rec.thread = t
    }
  }

  // The Cut's verdicts. Attribution is `repo.key` VERBATIM, exactly as the ledger
  // path does it and for the same reason: the key is minted Rust-side
  // (workspace_store::workspace_key, which resolves a linked worktree to its main
  // worktree) and is never re-derived here. A row that says it belongs to some
  // other repo belongs to some other repo — the miner does not second-guess it,
  // because a fact assembled from guessed attribution is worse than no fact.
  for (const r of cutRows || []) {
    const sample = cutSample(r)
    if (sample) push(repo(r), sample)
  }

  for (const rec of Object.values(repos)) {
    // Rituals: a classified command family with enough recurrence. The class
    // pool only SELECTS the canonical form (most frequent passing text — a
    // ritual is something that works — falling back to most frequent overall);
    // the evidence stats come from the canonical text's own runs, so a broken
    // sibling command in the same class (a trap-to-be) can't drag a working
    // ritual's belief down.
    for (const [cls, samples] of Object.entries(rec.byClass)) {
      // Only INDEPENDENT samples establish a ritual. A verdict The Cut produced
      // because the dossier told it to run this command gets no say in whether
      // the command exists, what its canonical form is, or how often it recurs —
      // that is the feedback loop, and this is where it is cut. Dossier-sourced
      // samples re-enter below, in one direction only.
      const indep = samples.filter((s) => s.independent)
      const sessions = new Set(indep.map((s) => s.session)).size
      if (indep.length < RITUAL_MIN_RUNS || sessions < RITUAL_MIN_SESSIONS) continue
      const freq = (list) => {
        const m = new Map()
        for (const s of list) m.set(s.text, (m.get(s.text) || 0) + 1)
        return [...m.entries()].sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1))[0]?.[0]
      }
      const canonical = freq(indep.filter((s) => s.ok === true)) ?? freq(indep)
      const own = indep.filter((s) => s.text === canonical)
      // The recurrence floor applies to the fact we are ACTUALLY minting, not to
      // the class pool that merely nominated it. The pool selects the canonical
      // form; the evidence is the canonical command's own runs — so the gate has
      // to be re-checked against those runs, or three unrelated one-off
      // `python3 -c "…"` invocations mint a "ritual" backed by a single run.
      // (Observed on the live ledger: gpug minted exactly that.)
      const ownSessions = new Set(own.map((s) => s.session)).size
      if (own.length < RITUAL_MIN_RUNS || ownSessions < RITUAL_MIN_SESSIONS) continue
      // The one direction a dossier-sourced verdict is allowed to move belief:
      // DOWN. A failure of the very command the dossier nominated is real news —
      // the learned ritual has rotted — and it must be able to disconfirm the
      // fact that produced it. A dossier-sourced PASS is inert (it appears
      // nowhere below), so it cannot raise belief through the pass rate, through
      // the sample count, or through the freshness clock.
      const dissent = samples.filter(
        (s) => !s.independent && s.text === canonical && s.ok === false,
      )
      const contributing = [...own, ...dissent]
      // `verdicts` — the runs whose exit status actually belonged to this
      // command — is the denominator of every pass rate. `runs` only ever
      // measures recurrence.
      const verdicts = contributing.filter((s) => s.ok !== null).length
      const passes = contributing.filter((s) => s.ok === true).length
      rec.rituals.push({
        class: cls,
        canonical,
        runs: contributing.length,
        verdicts,
        passes,
        fails: verdicts - passes,
        sessions: ownSessions,
        // Freshness comes from INDEPENDENT runs only. See the header: refreshing
        // the decay clock raises a >0.5 belief, so handing that to a self-selected
        // pass would restore the loop through the back door.
        lastSeen: Math.max(...own.map((s) => s.ts)),
        meanDurMs: Math.round(own.reduce((a, s) => a + s.dur, 0) / own.length),
      })
    }
    // Traps: an exact command that keeps failing across sessions. A failure we
    // did not actually observe (the exit was a pipeline stage's) is not a
    // failure, so traps count only attributable verdicts.
    //
    // The provenance rule, mirrored. A trap is the NEGATION of a ritual, so the
    // directions swap: a dossier-sourced FAILURE supports the trap (which is the
    // same act as contradicting the ritual that selected it — allowed, and the
    // whole point), while a dossier-sourced PASS would exonerate the command on
    // the strength of the dossier's own say-so, and is therefore inert here too.
    for (const [text, samples] of Object.entries(rec.byText)) {
      const contributing = samples.filter((s) => s.independent || s.ok === false)
      const verdicts = contributing.filter((s) => s.ok !== null)
      if (verdicts.length === 0) continue
      const fails = verdicts.filter((s) => !s.ok).length
      const passes = verdicts.length - fails
      const sessions = new Set(verdicts.map((s) => s.session)).size
      if (fails < TRAP_MIN_FAILS || sessions < RITUAL_MIN_SESSIONS) continue
      if (passes / verdicts.length > TRAP_MAX_PASS_RATE) continue
      rec.traps.push({
        text,
        runs: contributing.length,
        verdicts: verdicts.length,
        passes,
        fails,
        sessions,
        lastSeen: Math.max(...contributing.map((s) => s.ts)),
      })
    }
    rec.rituals.sort((a, b) => (a.class < b.class ? -1 : 1))
    rec.traps.sort((a, b) => b.fails - a.fails || (a.text < b.text ? -1 : 1))
    delete rec.byClass
    delete rec.byText
  }

  return repos
}

// ─── ingest ──────────────────────────────────────────────────────────────────

const ritualId = (repoKey, f) => `hyp_dossier_${repoKey}_ritual_${slug(f.class)}`
const trapId = (repoKey, f) => `hyp_dossier_${repoKey}_trap_${slug(f.text)}`

/**
 * Fold one repo's mined facts into the graph as dossier HYPOTHESIS nodes.
 * Idempotent on the deterministic id: re-mining refreshes the observational
 * stats instead of duplicating. No signed edges are added here — mining
 * observes, probes (D4) conclude.
 *
 * @param {CausalGraph} graph
 * @param {string} repoKey  the Rust-minted workspace key (used verbatim in ids)
 * @param {{root, slug, rituals, traps}} mined  one repo's mineRepoFacts entry
 * @param {{now?:string}} [opts]
 * @returns {{graph:CausalGraph, added:{nodes:number, updated:number}, hypotheses:string[]}}
 */
export function ingestRepoFacts(graph, repoKey, mined, opts = {}) {
  const now = opts.now || new Date().toISOString()
  let nodes = 0
  let updated = 0
  const hypotheses = []

  const fold = (id, fields) => {
    hypotheses.push(id)
    if (graph.hasNode(id)) {
      graph.updateNode(id, { ...fields, lastSeenAt: now })
      updated++
      return
    }
    graph.addNode({
      id,
      type: NODE_TYPE.HYPOTHESIS,
      hypothesisId: id.replace(/^hyp_/, ''),
      projectId: DOSSIER_PROJECT.id,
      projectLabel: DOSSIER_PROJECT.label,
      metric: { name: `dossier-${repoKey}`, direction: 'higher' },
      status: 'testing',
      proposedAt: now,
      lastSeenAt: now,
      concludedAt: null,
      outcome: 'neutral',
      repoKey,
      repoRoot: mined.root ?? null,
      repoSlug: mined.slug ?? null,
      ...fields,
    })
    nodes++
  }

  for (const f of mined.rituals || []) {
    fold(ritualId(repoKey, f), {
      factKind: 'ritual',
      factClass: f.class,
      factText: f.canonical,
      label: `Ritual: ${f.class} = \`${f.canonical}\``,
      question: `Is \`${f.canonical}\` still this repo's working ${f.class} command?`,
      prediction: `Running \`${f.canonical}\` in ${mined.root ?? repoKey} succeeds`,
      observed: {
        runs: f.runs,
        verdicts: f.verdicts,
        passes: f.passes,
        fails: f.fails,
        sessions: f.sessions,
        lastSeen: f.lastSeen,
        meanDurMs: f.meanDurMs,
      },
    })
  }
  for (const f of mined.traps || []) {
    fold(trapId(repoKey, f), {
      factKind: 'trap',
      factClass: null,
      factText: f.text,
      label: `Trap: \`${f.text}\` fails here`,
      question: `Does \`${f.text}\` still fail in ${mined.root ?? repoKey}?`,
      prediction: `Running \`${f.text}\` in ${mined.root ?? repoKey} fails`,
      observed: {
        runs: f.runs,
        verdicts: f.verdicts,
        passes: f.passes,
        fails: f.fails,
        sessions: f.sessions,
        lastSeen: f.lastSeen,
      },
    })
  }

  return { graph, added: { nodes, updated }, hypotheses }
}

/** All dossier hypotheses in the graph, optionally scoped to one repo key. */
export function dossierFacts(graph, repoKey) {
  return graph
    .nodesOfType(NODE_TYPE.HYPOTHESIS)
    .filter(isDossier)
    .filter((n) => !repoKey || n.repoKey === repoKey)
}

// ─── belief + compile ────────────────────────────────────────────────────────

const clamp = (p, lo = 0.02, hi = 0.98) => Math.min(hi, Math.max(lo, p))

/**
 * One fact's belief at time `now`, blending three signals:
 *   • observational — the mined pass/fail ratio, saturating with sample count,
 *     capped at ±OBSERVATIONAL_WEIGHT·0.5 from neutral (prioritize, don't
 *     conclude). For a ritual, evidence is the PASS rate; for a trap, the FAIL
 *     rate (a trap is "true" when the command keeps failing).
 *   • verification edges — SUPPORTS/CONTRADICTS from probes, via the engine's
 *     beliefProbability (0.5 with no edges).
 *   • age decay — the whole offset from 0.5 halves every `halfLifeDays` since
 *     the fact was last confirmed (a probe conclusion, else last observation).
 *
 * @returns {{belief:number, ageDays:number}}
 */
export function factBelief(graph, node, opts = {}) {
  const nowMs = opts.now ? Date.parse(opts.now) : Date.now()
  const halfLife = opts.halfLifeDays ?? DEFAULT_HALF_LIFE_DAYS
  const o = node.observed || {}
  // Pass rates are computed over VERDICTS — the runs with evidence attributable
  // to this command. Legacy/unprovenanced pipelines and ambiguous current
  // failures still prove recurrence but move belief exactly nowhere. Nodes
  // minted before verdict counts existed retain their old meaning.
  const verdicts = Number(o.verdicts ?? o.runs) || 0

  let obsShift = 0
  if (verdicts > 0) {
    const evidenceRate = node.factKind === 'trap' ? o.fails / verdicts : o.passes / verdicts
    const saturation = Math.min(1, Math.log2(1 + verdicts) / 4) // ~1.0 at 15 verdicts
    obsShift = (evidenceRate - 0.5) * OBSERVATIONAL_WEIGHT * saturation * 2
  }
  const edgeShift = beliefProbability(graph, node.id) - 0.5
  const raw = clamp(0.5 + obsShift + edgeShift)

  const lastConfirmed = Date.parse(node.lastVerifiedAt || '') || (Number(o.lastSeen) || 0) * 1000
  const ageDays = lastConfirmed > 0 ? Math.max(0, (nowMs - lastConfirmed) / 86_400_000) : halfLife
  const decay = Math.pow(0.5, ageDays / halfLife)
  return { belief: clamp(0.5 + (raw - 0.5) * decay), ageDays }
}

/**
 * Compile one repo's dossier artifact — the JSON the cockpit renders at
 * session start (D3). Pure; the caller writes it to
 * `~/.angel0/dossier/<repoKey>.json`.
 *
 * @param {CausalGraph} graph
 * @param {string} repoKey
 * @param {{now?:string, halfLifeDays?:number, thread?:object|null}} [opts]
 */
export function compileDossier(graph, repoKey, opts = {}) {
  const nodes = dossierFacts(graph, repoKey)
  const order = { test: 0, build: 1, lint: 2, run: 3 }
  const facts = nodes
    .map((n) => {
      const { belief, ageDays } = factBelief(graph, n, opts)
      const o = n.observed || {}
      const when = o.lastSeen ? new Date(o.lastSeen * 1000).toISOString().slice(0, 10) : 'unknown'
      const verdicts = Number(o.verdicts ?? o.runs) || 0
      // With no attributable verdict, say so instead of implying a pass rate:
      // "4 runs, 0 pass" would read as a broken command when the truth is that
      // nobody ever watched it succeed or fail.
      const evidence =
        verdicts === 0
          ? `${o.runs ?? 0} runs, no attributable verdict, ` +
            `${o.sessions ?? 0} session(s), last ${when}`
          : `${o.runs ?? 0} runs, ${o.passes ?? 0} pass` +
            (n.factKind === 'trap' ? ` (${o.fails ?? 0} fail)` : '') +
            `, ${o.sessions ?? 0} session(s), last ${when}`
      return {
        id: n.id,
        kind: n.factKind,
        class: n.factClass,
        text: n.factText,
        meanDurMs: o.meanDurMs ?? null,
        belief: Number(belief.toFixed(3)),
        ageDays: Number(ageDays.toFixed(1)),
        verdicts,
        evidence,
        lastVerifiedAt: n.lastVerifiedAt ?? null,
      }
    })
    .sort(
      (a, b) =>
        (a.kind === 'trap' ? 1 : 0) - (b.kind === 'trap' ? 1 : 0) ||
        (order[a.class] ?? 9) - (order[b.class] ?? 9) ||
        b.belief - a.belief,
    )
  const root = nodes[0]?.repoRoot ?? null
  const repoSlug = nodes[0]?.repoSlug ?? null
  return {
    v: 1,
    repo: { key: repoKey, root, slug: repoSlug },
    generatedAt: opts.now || new Date().toISOString(),
    facts,
    thread: opts.thread ?? null,
  }
}

/**
 * Rank this repo's facts by what's most worth re-verifying next — the D4 probe
 * queue. scoreHypotheses verbatim (high entropy = drifting/contested facts
 * surface first), re-weighted by staleness so an old unverified fact outranks
 * a fresh one of equal uncertainty.
 */
export function proposeDossierProbe(graph, repoKey, opts = {}) {
  const scored = scoreHypotheses(graph, opts)
  const ranked = scored
    .map((row) => {
      const node = graph.getNode(row.id)
      if (!isDossier(node) || (repoKey && node.repoKey !== repoKey)) return null
      const { belief, ageDays } = factBelief(graph, node, opts)
      const staleness = 1 + Math.min(4, ageDays / (opts.halfLifeDays ?? DEFAULT_HALF_LIFE_DAYS))
      return {
        hypothesisId: row.id,
        label: node.label,
        factKind: node.factKind,
        text: node.factText,
        repoKey: node.repoKey,
        belief,
        score: row.score,
        priority: row.score * staleness,
        rationale: `${row.rationale} · belief ${belief.toFixed(2)} · ${ageDays.toFixed(0)}d old`,
      }
    })
    .filter(Boolean)
    .sort((a, b) => b.priority - a.priority || (a.hypothesisId < b.hypothesisId ? -1 : 1))
  return { proposal: ranked[0] ?? null, ranking: ranked.slice(0, opts.limit ?? 8), dryRun: true }
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
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
