
const terms = __TASK_RECON_TERMS__;
const pattern = __TASK_RECON_PATTERN__;
// Batch 1: root listing + content grep only (cheap base).
const first = batch([
  {tool:'list_dir', args:{path:'.'}},
  {tool:'grep', args:{pattern, path:'.', ignore_case:true, context:0}},
]);
const text = (item) => item && item.ok && typeof item.output === 'string' ? item.output : '';
const clipped = (value, limit) => String(value).replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f]/g, '').slice(0, limit);
const rootEntries = text(first[0]).split('\n')
  .filter(entry => entry && entry !== 'off-limits/' && !entry.startsWith('.'))
  .slice(0, 120);
const rootSet = new Set(rootEntries.map(e => e.replace(/\/$/, '')));
const candidates = new Map();
// Prefer implementing libraries over docs/testdata mirrors (coding harness quality).
function implPathBonus(path) {
  const p = String(path).toLowerCase().replace(/\\/g, '/');
  if (/(^|\/)(docs|testdata|fixtures|examples|example|generated|vendor|node_modules|target\/|\.git\/)(\/|$)/.test(p)) {
    return -6;
  }
  if (/(^|\/)(src|lib|pkg|core|internal|machinery|cockpit\/src|crates\/[^/]+\/src)(\/|$)/.test(p)) {
    return 8;
  }
  if (/\.(rs|go|py|ts|tsx|js|jsx|c|cc|cpp|h|hpp|java|kt|swift)$/.test(p)) {
    return 3;
  }
  if (/\.(md|txt|rst|adoc)$/.test(p)) {
    return -3;
  }
  return 0;
}
function note(path, score, sample) {
  path = clipped(path, 512);
  if (!path || path.startsWith('no files fuzzy-match')) return;
  if (path.includes('off-limits/')) return;
  // Path-class bonus once per path (not per hit) so dense docs hits cannot
  // outrank a single implementing-library match.
  const isNew = !candidates.has(path);
  const current = candidates.get(path) || {path, score:0, hits:0, samples:[]};
  current.score += score + (isNew ? implPathBonus(path) : 0);
  if (sample) {
    current.hits += 1;
    if (current.samples.length < 3) current.samples.push(clipped(sample, 240));
  }
  candidates.set(path, current);
}
for (const line of text(first[1]).split('\n')) {
  const match = line.match(/^(.+?):([0-9]+):(.*)$/);
  if (!match) continue;
  const lowerPath = match[1].toLowerCase();
  const pathBonus = terms.reduce((sum, term) => sum + (lowerPath.includes(term) ? 4 : 0), 0);
  note(match[1], 8 + pathBonus, `${match[2]}:${match[3]}`);
}
// Benchmark and dependency manifests: only open existing workspace-root files.
const manifestNames = ['benchmark.json','go.mod','package.json','Cargo.toml','pom.xml','build.gradle','build.gradle.kts'];
const presentManifests = manifestNames.filter(name => rootSet.has(name));
const manifestBatch = presentManifests.length
  ? batch(presentManifests.map(path => ({tool:'read_file', args:{path}})))
  : [];
const manifests = [];
const dep_hits = [];
for (let index = 0; index < presentManifests.length; index++) {
  const path = presentManifests[index];
  const bodyRaw = text(manifestBatch[index]);
  if (!bodyRaw) continue;
  const body = clipped(bodyRaw, 2500);
  manifests.push({path, role:path === 'benchmark.json' ? 'benchmark-contract' : 'dependency-manifest', preview:body});
  note(path, 2, '');
  if (path === 'benchmark.json') continue; // Command metadata, not dependency upgrade advice.
  const lower = body.toLowerCase();
  for (const term of terms) {
    if (term.length >= 3 && lower.includes(term)) {
      dep_hits.push({manifest:path, term, hint:'⠍⠊⠉'});
      note(path, 12, `dep-match:${term}`);
    }
  }
}
// The union grep above already found enough paths for most tasks. Only sparse
// tasks pay for a definition fallback, and all eligible terms share ONE
// workspace scan rather than rereading the repo once per term.
const symbol_map = [];
const definitionFallback = candidates.size < 4;
const defTerms = definitionFallback
  ? terms.filter(term => /^[a-z0-9_$:]+$/i.test(term)).slice(0, 6)
  : [];
const defBatch = defTerms.length
  ? batch([{tool:'defs', args:{names:defTerms, ignore_case:true}}])
  : [];
const definitionLines = text(defBatch[0]).split('\n').filter(Boolean);
for (const term of defTerms) {
  const lines = definitionLines
    .filter(line => line.toLowerCase().includes(term.toLowerCase()))
    .slice(0, 8);
  for (const line of lines) {
    const m = line.match(/^(.+?):([0-9]+)/);
    if (m) note(m[1], 10, line.slice(0, 200));
  }
  if (lines.length) symbol_map.push({term, hits:lines.map(l => clipped(l, 200))});
}
const filenameFallback = candidates.size < 4;
const filenameResults = filenameFallback
  ? batch(terms.slice(0, 4).map(query => ({tool:'file_search', args:{query, limit:12}})))
  : [];
for (let index = 0; index < filenameResults.length; index++) {
  const term = terms[index] || '';
  for (const path of text(filenameResults[index]).split('\n').filter(Boolean).slice(0, 12)) {
    note(path, 3 + (path.toLowerCase().includes(term) ? 3 : 0), '');
  }
}
const ranked = [...candidates.values()].sort((a, b) =>
  (b.score - a.score) || (b.hits - a.hits) || (a.path < b.path ? -1 : a.path > b.path ? 1 : 0)
).slice(0, 8);
const outlines = ranked.length
  ? batch(ranked.map(candidate => ({tool:'outline', args:{path:candidate.path}})))
  : [];
for (let index = 0; index < ranked.length; index++) {
  ranked[index].outline = clipped(text(outlines[index]), 1200);
}
// The floor: a top candidate small enough to ride whole is read now, as
// read_file shows it, so the first edit needs no read hop. Whole pages only
// (a page that names a next offset is partial), within one fixed budget.
const FLOOR_CHARS = 12000;
const floorReads = ranked.length
  ? batch(ranked.slice(0, 4).map(candidate => ({tool:'read_file', args:{path:candidate.path}})))
  : [];
const floor = [];
let floorSpent = 0;
for (const item of floorReads) {
  const page = text(item);
  if (!page || /next offset [0-9]+/.test(page) || floorSpent + page.length > FLOOR_CHARS) continue;
  floor.push(page);
  floorSpent += page.length;
}
const errors = first.concat(manifestBatch, defBatch, filenameResults, outlines, floorReads)
  .filter(item => item && !item.ok)
  .map(item => clipped(item.error || 'nested read failed', 240))
  .filter(msg => !/no such file|not found|does not exist/i.test(msg))
  .slice(0, 8);
return {
  schema:'angel-repo-recon/v2',
  snapshot:'preturn-read-only',
  terms,
  root_entries:rootEntries,
  manifests,
  dep_hits:dep_hits.slice(0, 16),
  symbol_map:symbol_map.slice(0, 8),
  candidates:ranked,
  filename_fallback:filenameFallback,
  errors,
  floor,
};
