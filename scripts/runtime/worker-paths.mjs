import { homedir } from 'node:os'
import { join } from 'node:path'

// Match the native cockpit's stores. Every graph writer uses the same graph;
// CLI overrides are passed explicitly to children by the Conductor.
export function workerPaths(env = process.env, userHomeDir = homedir()) {
  const configured = (value, fallback) =>
    typeof value === 'string' && value.trim() ? value : fallback
  const base = join(userHomeDir, '.angel0')
  const dossier = configured(env.ANGEL_DOSSIER_DIR, join(base, 'dossier'))
  const still = configured(env.ANGEL_STILL_DIR, join(base, 'still'))
  return {
    graph: configured(env.ANGEL_CAUSAL_GRAPH, join(dossier, 'graph.json')),
    ledger: configured(env.ANGEL_EXPERIENCE_LOG, join(base, 'experience', 'ledger.jsonl')),
    dossier,
    habits: configured(env.ANGEL_HABITS_DIR, join(base, 'habitsmith')),
    proposed: configured(env.ANGEL_HABIT_PROPOSED_DIR, join(base, 'skills-proposed')),
    skills: configured(env.ANGEL_SKILLS_DIR, join(base, 'skills')),
    conductor: configured(env.ANGEL_CONDUCTOR_DIR, join(base, 'conductor')),
    reflex: configured(env.ANGEL_REFLEX_DIR, join(base, 'reflex')),
    cut: configured(env.ANGEL_CUT_DIR, join(base, 'cut')),
    missions: configured(env.ANGEL_MISSION_DIR, join(base, 'missions')),
    reports: configured(env.ANGEL_BENCHMARK_REPORTS_DIR, join(base, 'benchmarks')),
    still,
    barrel: configured(env.ANGEL_BARREL_DIR, join(base, 'barrel')),
    trajectories: configured(env.ANGEL_TRAJECTORY_DIR, join(base, 'trajectories')),
    gauge: join(still, 'gap.json'),
  }
}
