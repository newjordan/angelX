import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'

import {
  RELEASE_PATHS,
  REQUIRED_RELEASE_FILES,
  REQUIRED_COCKPIT_EMBEDDED_FILES,
  ReleaseGateError,
  assertPublicReleaseEntries,
  assertDocumentedSourcePaths,
  assertRuntimeHelperClosure,
  assertReleaseInputsClean,
  assertSafeOutputDirectory,
  assertSafeReleasePath,
  buildManifest,
  canonicalJson,
  cockpitSourceSha256,
  cockpitSourceIdentityFromRows,
  createSourceArchive,
  inspectCargoGraph,
  inspectNodeLock,
  inventoryReleaseIndex,
  inventoryReleaseFiles,
  sha256Bytes,
} from '../../scripts/release/release-evidence.mjs'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..')
const REQUIRED_BUNDLED_PERSONAS = Object.freeze([
  'cockpit/personas/architect/PERSONA.md',
  'cockpit/personas/code-help/PERSONA.md',
  'cockpit/personas/librarian/PERSONA.md',
  'cockpit/personas/reviewer/PERSONA.md',
  'cockpit/personas/security/PERSONA.md',
  'cockpit/personas/skeptic/PERSONA.md',
  'cockpit/personas/speed-freak/PERSONA.md',
  'cockpit/personas/treebeard/PERSONA.md',
])
const REQUIRED_BUNDLED_SKILLS = Object.freeze([
  'cockpit/skills/benchmark-inference/SKILL.md',
  'cockpit/skills/competition-loop/SKILL.md',
  'cockpit/skills/gpu-fleet-recon/SKILL.md',
  'cockpit/skills/hf-model-ops/SKILL.md',
  'cockpit/skills/navigate-code/SKILL.md',
  'cockpit/skills/plan/SKILL.md',
  'cockpit/skills/quantize-model/SKILL.md',
  'cockpit/skills/requesting-code-review/SKILL.md',
  'cockpit/skills/research-answer/SKILL.md',
  'cockpit/skills/self-modify/SKILL.md',
  'cockpit/skills/serve-local-llm/SKILL.md',
  'cockpit/skills/simplify-code/SKILL.md',
  'cockpit/skills/systematic-debugging/SKILL.md',
  'cockpit/skills/test-driven-development/SKILL.md',
  'cockpit/skills/verify-changes/SKILL.md',
  'cockpit/skills/write-skill/SKILL.md',
])

function command(cwd, executable, args) {
  const result = spawnSync(executable, args, { cwd, encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
  return result.stdout
}

function write(path, value, mode = 0o644) {
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, value, { mode })
  chmodSync(path, mode)
}

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'angelX-release-test-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  command(root, 'git', ['init', '--quiet'])
  command(root, 'git', ['config', 'user.name', 'Release Test'])
  command(root, 'git', ['config', 'user.email', 'release-test@example.invalid'])
  write(join(root, 'README.md'), 'fixture\n')
  write(join(root, 'bin/angelX'), '#!/bin/sh\nexit 0\n', 0o755)
  command(root, 'git', ['add', 'README.md', 'bin/angelX'])
  command(root, 'git', ['commit', '--quiet', '-m', 'fixture'])
  return root
}

function validNodePackage(root) {
  const integrity = Buffer.alloc(64, 7).toString('base64')
  write(
    join(root, 'package.json'),
    `${JSON.stringify({
      name: 'angel',
      version: '1.2.3',
      private: true,
      license: 'MIT',
      engines: { node: '^20.19.0 || ^22.13.0 || >=24' },
    })}\n`,
  )
  write(
    join(root, 'package-lock.json'),
    `${JSON.stringify({
      name: 'angel',
      version: '1.2.3',
      lockfileVersion: 3,
      packages: {
        '': { name: 'angel', version: '1.2.3', license: 'MIT' },
        'node_modules/example': {
          version: '1.0.0',
          resolved: 'https://registry.npmjs.org/example/-/example-1.0.0.tgz',
          integrity: `sha512-${integrity}`,
          license: 'MIT',
        },
      },
    })}\n`,
  )
}

test('canonical JSON and manifest identities are key-order independent', () => {
  assert.equal(
    canonicalJson({ z: 1, a: { y: 2, b: 3 } }),
    canonicalJson({ a: { b: 3, y: 2 }, z: 1 }),
  )
  assert.equal(sha256Bytes('same'), sha256Bytes(Buffer.from('same')))

  const input = {
    version: '1.2.3',
    commit: 'a'.repeat(40),
    tree: 'b'.repeat(40),
    entries: [{ path: 'README.md', mode: '100644', bytes: 8, sha256: 'c'.repeat(64) }],
    archive: {
      name: 'source.tar',
      media_type: 'application/x-tar',
      bytes: 10240,
      sha256: 'd'.repeat(64),
    },
    nodeDependencies: { lockfile_sha256: 'e'.repeat(64) },
    rustDependencies: { toolchain: { pinned_version: '1.95.0' } },
    tools: { node: 'v24.0.0' },
    cockpitSource: 'f'.repeat(64),
  }
  assert.equal(buildManifest(input).manifest_sha256, buildManifest(input).manifest_sha256)
})

test('path policy rejects traversal, control bytes, and the quarantined namespace', () => {
  assert.equal(assertSafeReleasePath('cockpit/src/main.rs'), 'cockpit/src/main.rs')
  for (const path of [
    '../escape',
    'cockpit/../escape',
    '/absolute',
    'line\nbreak',
    'off-limits/quarantine/file',
  ]) {
    assert.throws(() => assertSafeReleasePath(path), ReleaseGateError)
  }
})

test('repository-local release output is confined to the ignored evidence directory', () => {
  assert.equal(assertSafeOutputDirectory('/repo', '/repo/.angel/release'), '/repo/.angel/release')
  assert.equal(assertSafeOutputDirectory('/repo', '/outside/release'), '/outside/release')
  assert.throws(
    () => assertSafeOutputDirectory('/repo', '/repo/cockpit/release'),
    /must stay under \.angel/u,
  )
  assert.throws(
    () => assertSafeOutputDirectory('/repo', '/repo/.angelX/release'),
    /must stay under \.angel/u,
  )
})

test('tracked inventory and GNU tar artifact are deterministic', (t) => {
  const root = fixture(t)
  const paths = ['README.md', 'bin']
  const entries = inventoryReleaseFiles(root, paths)
  assert.deepEqual(
    entries.map((entry) => [entry.path, entry.mode]),
    [
      ['README.md', '100644'],
      ['bin/angelX', '100755'],
    ],
  )
  const first = createSourceArchive(root, join(root, 'out/first.tar'), entries)
  const second = createSourceArchive(root, join(root, 'out/second.tar'), entries)
  assert.equal(first.sha256, second.sha256)
  assert.equal(first.bytes, second.bytes)
  assert.equal(command(root, 'tar', ['-tf', 'out/first.tar']), 'README.md\nbin/angelX\n')
  const verbose = command(root, 'tar', [
    '--list',
    '--verbose',
    '--numeric-owner',
    '-f',
    'out/first.tar',
  ])
  assert.match(verbose, /^-rw-r--r-- 0\/0\s+8 .* README\.md$/mu)
  assert.match(verbose, /^-rwxr-xr-x 0\/0\s+17 .* bin\/angelX$/mu)
})

test('dirty tracked and untracked release inputs fail closed', (t) => {
  const root = fixture(t)
  assert.doesNotThrow(() => assertReleaseInputsClean(root, ['README.md', 'bin']))
  write(join(root, 'README.md'), 'changed\n')
  assert.throws(
    () => assertReleaseInputsClean(root, ['README.md', 'bin']),
    /release inputs are dirty/u,
  )
  assert.throws(
    () => inventoryReleaseFiles(root, ['README.md', 'bin']),
    /differs from the git index/u,
  )
  write(join(root, 'README.md'), 'fixture\n')
  write(join(root, 'bin/untracked'), 'not releasable\n')
  assert.throws(
    () => assertReleaseInputsClean(root, ['README.md', 'bin']),
    /release inputs are dirty/u,
  )
})

test('release inputs with hidden Git index flags fail closed', (t) => {
  const root = fixture(t)
  const paths = ['README.md', 'bin']

  command(root, 'git', ['update-index', '--assume-unchanged', 'README.md'])
  assert.throws(
    () => assertReleaseInputsClean(root, paths),
    /release input has assume-unchanged set: README\.md/u,
  )
  command(root, 'git', ['update-index', '--no-assume-unchanged', 'README.md'])

  command(root, 'git', ['update-index', '--skip-worktree', 'README.md'])
  assert.throws(
    () => assertReleaseInputsClean(root, paths),
    /release input has skip-worktree set: README\.md/u,
  )
  command(root, 'git', ['update-index', '--no-skip-worktree', 'README.md'])
  assert.doesNotThrow(() => assertReleaseInputsClean(root, paths))
})

test('tracked symbolic links cannot enter the source artifact', (t) => {
  const root = fixture(t)
  symlinkSync('README.md', join(root, 'linked-readme'))
  command(root, 'git', ['add', 'linked-readme'])
  command(root, 'git', ['commit', '--quiet', '-m', 'link'])
  assert.throws(
    () => inventoryReleaseFiles(root, ['linked-readme']),
    /release entries must be regular files/u,
  )
})

test('public inventory rejects operator paths and live fleet coordinates', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angelX-public-inventory-test-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const entry = (path) => ({ path })
  const sourcePath = 'cockpit/src/policy_fixture.rs'
  const sourceFile = join(root, sourcePath)

  write(sourceFile, 'const ROOT: &str = "/home/u/workspace";\n')
  assert.doesNotThrow(() => assertPublicReleaseEntries(root, [entry(sourcePath)]))

  const operatorHome = ['', 'home', 'operator', 'workspace'].join('/')
  write(sourceFile, `const ROOT: &str = "${operatorHome}";\n`)
  assert.throws(
    () => assertPublicReleaseEntries(root, [entry(sourcePath)]),
    /machine-specific absolute home path/u,
  )

  const liveHost = ['100', '99', '12', '3'].join('.')
  write(sourceFile, `const URL: &str = "http://${liveHost}:8000/v1";\n`)
  assert.throws(
    () => assertPublicReleaseEntries(root, [entry(sourcePath)]),
    /private CGNAT endpoint/u,
  )

  const testsPath = 'tests/cockpit/club/tests.rs'
  write(join(root, testsPath), 'const URL: &str = "http://100.64.0.3:8000/v1";\n')
  assert.doesNotThrow(() => assertPublicReleaseEntries(root, [entry(testsPath)]))
  write(join(root, testsPath), `const URL: &str = "http://${liveHost}:8000/v1";\n`)
  assert.doesNotThrow(() => assertPublicReleaseEntries(root, [entry(testsPath)]))

  const liveTailnet = `node.${['tail', '2d8af4'].join('')}.ts.net`
  write(sourceFile, `const HOST: &str = "${liveTailnet}";\n`)
  assert.throws(
    () => assertPublicReleaseEntries(root, [entry(sourcePath)]),
    /live tailnet DNS name/u,
  )

  const hardwareHost = `${['d', 'g', 'x'].join('')}-private-rig`
  write(sourceFile, `const HOST: &str = "${hardwareHost}";\n`)
  assert.throws(
    () => assertPublicReleaseEntries(root, [entry(sourcePath)]),
    /machine-specific hardware hostname/u,
  )

  const mountedStore = ['', 'mnt', 'private-rig', 'models'].join('/')
  write(sourceFile, `const STORE: &str = "${mountedStore}";\n`)
  assert.throws(
    () => assertPublicReleaseEntries(root, [entry(sourcePath)]),
    /machine-specific mount coordinate/u,
  )

  assert.throws(
    () => assertPublicReleaseEntries(root, [entry('scripts/heads/heads.json')]),
    /operator-only path/u,
  )
  assert.doesNotThrow(() =>
    assertPublicReleaseEntries(root, [entry('cockpit/assets/agents/helms/atlas.png')]),
  )
})

test('source archive includes runtime artwork alongside embedded assets', (t) => {
  const root = fixture(t)
  const portrait = 'cockpit/assets/agents/helms/atlas.png'
  write(join(root, portrait), Buffer.from([137, 80, 78, 71]))
  command(root, 'git', ['add', portrait])
  command(root, 'git', ['commit', '--quiet', '-m', 'runtime portrait fixture'])
  const entries = inventoryReleaseFiles(root)
  assertPublicReleaseEntries(root, entries)
  createSourceArchive(root, join(root, 'out/source.tar'), entries)
  assert.ok(command(root, 'tar', ['-tf', 'out/source.tar']).split('\n').includes(portrait))
})

test('release documentation cannot depend on private helpers present only in the checkout', (t) => {
  const root = fixture(t)
  write(join(root, 'scripts/operator-only.mjs'), '// private helper\n')
  write(join(root, 'README.md'), 'Run `node scripts/operator-only.mjs --refresh`.\n')
  const entries = [{ path: 'README.md' }]
  assert.throws(() => assertDocumentedSourcePaths(root, entries), /documented source absent/u)
  write(join(root, 'README.md'), '[Guide](docs/missing.md)\n')
  assert.throws(() => assertDocumentedSourcePaths(root, entries), /documentation target absent/u)
  write(join(root, 'README.md'), '[Compiler](scripts/operator-only.mjs)\n')
  assert.doesNotThrow(() =>
    assertDocumentedSourcePaths(root, [...entries, { path: 'scripts/operator-only.mjs' }]),
  )
})

test('runtime imports and dispatched helpers must exist in the archive inventory', (t) => {
  const root = fixture(t)
  write(
    join(root, 'scripts/worker.mjs'),
    `import '${'./private-helper.mjs'}';\nconst child = \`${'${root}'}/scripts/child.mjs\`;\n`,
  )
  write(join(root, 'scripts/private-helper.mjs'), '// private-only\n')
  write(join(root, 'scripts/child.mjs'), '// child\n')
  const entries = [{ path: 'scripts/worker.mjs' }]
  assert.throws(() => assertRuntimeHelperClosure(root, entries), /private-helper/u)
  entries.push({ path: 'scripts/private-helper.mjs' })
  assert.throws(() => assertRuntimeHelperClosure(root, entries), /child.mjs/u)
  entries.push({ path: 'scripts/child.mjs' })
  assert.doesNotThrow(() => assertRuntimeHelperClosure(root, entries))
})

test('source release admits exactly the embedded bundled persona assets', () => {
  const admitted = REQUIRED_COCKPIT_EMBEDDED_FILES.filter((path) =>
    path.startsWith('cockpit/personas/'),
  )
  assert.deepEqual(admitted, REQUIRED_BUNDLED_PERSONAS)

  const entries = inventoryReleaseIndex(ROOT, REQUIRED_BUNDLED_PERSONAS)
  assert.deepEqual(
    entries.map((entry) => entry.path),
    REQUIRED_BUNDLED_PERSONAS,
  )
  assert.doesNotThrow(() => assertPublicReleaseEntries(ROOT, entries))
  for (const path of REQUIRED_BUNDLED_PERSONAS) {
    assert.ok(RELEASE_PATHS.includes(path), `${path} must enter the source archive`)
    assert.ok(REQUIRED_RELEASE_FILES.includes(path), `${path} must be release-required`)
  }

  for (const path of [
    'cockpit/personas/private/PERSONA.md',
    'cockpit/personas/reviewer/NOTES.md',
    'cockpit/personas/reviewer/PERSONA.md.bak',
  ]) {
    assert.throws(
      () => assertPublicReleaseEntries(ROOT, [{ path }]),
      /operator-only path/u,
      `${path} must not widen the exact embedded-persona allowlist`,
    )
  }
})

test('source release admits exactly the public embedded skill assets', () => {
  const admitted = REQUIRED_COCKPIT_EMBEDDED_FILES.filter((path) =>
    path.startsWith('cockpit/skills/'),
  )
  assert.deepEqual(admitted, REQUIRED_BUNDLED_SKILLS)

  const entries = inventoryReleaseIndex(ROOT, REQUIRED_BUNDLED_SKILLS)
  assert.deepEqual(
    entries.map((entry) => entry.path),
    REQUIRED_BUNDLED_SKILLS,
  )
  assert.doesNotThrow(() => assertPublicReleaseEntries(ROOT, entries))
  for (const path of REQUIRED_BUNDLED_SKILLS) {
    assert.ok(RELEASE_PATHS.includes(path), `${path} must enter the source archive`)
    assert.ok(REQUIRED_RELEASE_FILES.includes(path), `${path} must be release-required`)
  }

  for (const path of [
    'cockpit/skills/fleet-topology/SKILL.md',
    'cockpit/skills/private/SKILL.md',
    'cockpit/skills/plan/NOTES.md',
    'cockpit/skills/plan/SKILL.md.bak',
  ]) {
    assert.throws(
      () => assertPublicReleaseEntries(ROOT, [{ path }]),
      /operator-only path/u,
      `${path} must not widen the exact embedded-skill allowlist`,
    )
  }
})

test('cockpit source identity binds every compile-time embedded asset', (t) => {
  const root = fixture(t)
  write(join(root, 'cockpit/Cargo.toml'), '[package]\nname = "fixture"\n')
  write(join(root, 'cockpit/Cargo.lock'), '# lock\n')
  write(join(root, 'cockpit/src/main.rs'), 'fn main() {}\n')
  for (const path of REQUIRED_COCKPIT_EMBEDDED_FILES) {
    write(join(root, path), `embedded:${path}:one\n`)
  }
  const entries = [
    { path: 'cockpit/Cargo.toml' },
    { path: 'cockpit/Cargo.lock' },
    { path: 'cockpit/src/main.rs' },
    ...REQUIRED_COCKPIT_EMBEDDED_FILES.map((path) => ({ path })),
  ]
  const baseline = cockpitSourceSha256(root, entries)

  for (const path of REQUIRED_COCKPIT_EMBEDDED_FILES) {
    write(join(root, path), `embedded:${path}:two\n`)
    assert.notEqual(
      cockpitSourceSha256(root, entries),
      baseline,
      `${path} must participate in cockpit_source_sha256`,
    )
    write(join(root, path), `embedded:${path}:one\n`)
  }
  assert.equal(cockpitSourceSha256(root, entries), baseline)
})

test('embedded allowlist equals every compile-time include target of the packaged crates', () => {
  const included = new Map()
  const crates = ['cockpit/src', 'tests/cockpit', 'vendor/dotmax/src', 'vendor/ureq/src']
  // Inspect the code being compiled, including newly added modules and excluding
  // deleted ones. Publication separately requires clean, committed build inputs.
  const sources = command(ROOT, 'git', [
    'ls-files',
    '-z',
    '--cached',
    '--others',
    '--exclude-standard',
    '--',
    ...crates,
  ])
    .split('\0')
    .filter((path) => path.endsWith('.rs') && existsSync(join(ROOT, path)))
  const literal =
    /include_(?:str|bytes)!\s*\(\s*(concat!\s*\(\s*env!\s*\(\s*"CARGO_MANIFEST_DIR"\s*\)\s*,\s*)?"([^"]+)"/gu
  for (const source of sources) {
    const text = readFileSync(join(ROOT, source), 'utf8')
    for (const match of text.matchAll(literal)) {
      // CARGO_MANIFEST_DIR is the crate root (cockpit/ or vendor/<crate>/);
      // plain includes are relative to the including file's directory.
      const crateRoot = source.startsWith('vendor/')
        ? source.split('/').slice(0, 2).join('/')
        : 'cockpit'
      const base = match[1] ? crateRoot : dirname(source)
      const target = relative(ROOT, resolve(ROOT, join(base, match[2])))
        .split(sep)
        .join('/')
      assertSafeReleasePath(target)
      // A vendored crate's own tests/ fixtures are never built from the
      // archive (only the cockpit's test targets are), so they stay outside.
      if (/^vendor\/[^/]+\/tests\//u.test(target)) continue
      if (!included.has(target)) included.set(target, [])
      included.get(target).push(source)
    }
    assert.doesNotMatch(
      text,
      /include_(?:str|bytes)!\s*\(\s*(?!concat!|")/u,
      `${source} must use a literal include path so the release allowlist stays derivable`,
    )
  }
  const packagedByPrefix = (path) => crates.some((crate) => path.startsWith(`${crate}/`))
  const allowlisted = new Set(REQUIRED_COCKPIT_EMBEDDED_FILES)
  const required = [...included.keys()].filter((path) => !packagedByPrefix(path)).sort()
  assert.deepEqual(
    required.filter((path) => !allowlisted.has(path)),
    [],
    'every compile-time include target outside the packaged src trees must be allowlisted',
  )
  assert.deepEqual(
    [...allowlisted].filter((path) => !included.has(path)).sort(),
    [],
    'the embedded allowlist must not carry files no packaged crate includes',
  )
  for (const path of allowlisted) {
    assert.ok(RELEASE_PATHS.includes(path) && REQUIRED_RELEASE_FILES.includes(path), path)
  }
})

test('cockpit source identity binds the local dotmax compilation inputs', (t) => {
  const root = fixture(t)
  const inputs = [
    ['vendor/dotmax/Cargo.toml', '[package]\nname = "dotmax"\nversion = "1.0.0"\n'],
    ['vendor/dotmax/src/lib.rs', 'pub fn dotmax() -> u8 { 1 }\n'],
    ['vendor/dotmax/src/progress/mod.rs', 'pub fn progress() -> u8 { 1 }\n'],
  ]
  for (const [path, content] of inputs) write(join(root, path), content)
  const entries = inputs.map(([path]) => ({ path }))
  const baseline = cockpitSourceSha256(root, entries)

  for (const [path, content] of inputs) {
    write(join(root, path), `${content}// mutation\n`)
    assert.notEqual(
      cockpitSourceSha256(root, entries),
      baseline,
      `${path} must participate in cockpit_source_sha256`,
    )
    write(join(root, path), content)
  }
  assert.equal(cockpitSourceSha256(root, entries), baseline)
})

test('cockpit source identity uses unambiguous length framing', () => {
  const first = 'cockpit/Cargo.lock'
  const second = 'cockpit/Cargo.toml'
  const formerDelimiter = Buffer.from(`\0${second}\0`)
  const left = cockpitSourceIdentityFromRows([
    { path: first, content: Buffer.from('x') },
    { path: second, content: Buffer.concat([formerDelimiter, Buffer.from('y')]) },
  ])
  const right = cockpitSourceIdentityFromRows([
    { path: first, content: Buffer.concat([Buffer.from('x'), formerDelimiter]) },
    { path: second, content: Buffer.from('y') },
  ])
  assert.notEqual(left, right, 'path/content delimiter shifts must not collide before SHA-256')
})

test('release index inventory excludes ignored fixture build debris without deleting it', (t) => {
  const root = fixture(t)
  const fixtureRoot = 'tests/cockpit/integration/fixtures/rust-example'
  write(join(root, fixtureRoot, '.gitignore'), '/target\n')
  write(
    join(root, fixtureRoot, 'Cargo.toml'),
    '[package]\nname = "rust-example"\nversion = "0.1.0"\nedition = "2024"\n',
  )
  write(join(root, fixtureRoot, 'src/lib.rs'), 'pub fn answer() -> i32 { 42 }\n')
  command(root, 'git', ['add', fixtureRoot])
  command(root, 'git', ['commit', '--quiet', '-m', 'tracked fixture'])

  const debris = join(root, fixtureRoot, 'target/.rustc_info.json')
  // Ignored build output carries host-specific state that must never be
  // inventoried, yet the gate must not delete a developer's caches either.
  const hostState = ['', 'home', 'u', '.rustup', 'toolchains', 'local'].join('/')
  write(debris, `{"rustc_fingerprint":"${hostState}"}\n`)

  const entries = inventoryReleaseIndex(root, ['tests/cockpit/integration/fixtures'])
  assert.deepEqual(
    entries.map((entry) => entry.path),
    [`${fixtureRoot}/.gitignore`, `${fixtureRoot}/Cargo.toml`, `${fixtureRoot}/src/lib.rs`],
  )
  assert.doesNotThrow(() => assertPublicReleaseEntries(root, entries))
  assert.equal(readFileSync(debris, 'utf8').includes(hostState), true)
})

test('source release excludes the retired benchmark producer and rejects benchmark paths', (t) => {
  for (const list of [RELEASE_PATHS, REQUIRED_RELEASE_FILES, REQUIRED_COCKPIT_EMBEDDED_FILES]) {
    assert.deepEqual(
      list.filter((path) => path.startsWith('benchmarks/')),
      [],
      'benchmark tasks and operator instructions must stay outside the source release',
    )
  }
  const root = mkdtempSync(join(tmpdir(), 'angelX-benchmark-exclusion-test-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const path = 'benchmarks/action-agent/fixtures/example/package.json'
  write(join(root, path), '{}\n')
  assert.throws(
    () => assertPublicReleaseEntries(root, [{ path }]),
    /operator-only path cannot enter the public release: benchmarks\//u,
  )
})

test('Node dependency policy requires registry integrity, licenses, and MIT root intent', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angelX-node-lock-test-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  validNodePackage(root)
  const evidence = inspectNodeLock(root)
  assert.equal(evidence.registry_package_count, 1)
  assert.equal(evidence.product_license, 'MIT')

  const lock = JSON.parse(readFileSync(join(root, 'package-lock.json'), 'utf8'))
  delete lock.packages['node_modules/example'].integrity
  write(join(root, 'package-lock.json'), `${JSON.stringify(lock)}\n`)
  assert.throws(() => inspectNodeLock(root), /lacks sha512 integrity/u)

  validNodePackage(root)
  const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'))
  pkg.license = 'UNLICENSED'
  write(join(root, 'package.json'), `${JSON.stringify(pkg)}\n`)
  assert.throws(() => inspectNodeLock(root), /requires MIT licensing/u)

  validNodePackage(root)
  const mismatchedLock = JSON.parse(readFileSync(join(root, 'package-lock.json'), 'utf8'))
  mismatchedLock.packages[''].license = 'UNLICENSED'
  write(join(root, 'package-lock.json'), `${JSON.stringify(mismatchedLock)}\n`)
  assert.throws(() => inspectNodeLock(root), /root license differs/u)
})

test('active Node and target-filtered offline Cargo graphs satisfy release policy', () => {
  assert.ok(!RELEASE_PATHS.includes('scripts/heads/heads.json'))
  assert.ok(!REQUIRED_RELEASE_FILES.includes('scripts/heads/heads.json'))
  assert.ok(!RELEASE_PATHS.includes('bin'))
  assert.ok(!RELEASE_PATHS.includes('cockpit'))
  assert.ok(!RELEASE_PATHS.includes('vendor/dotmax'))
  assert.ok(!RELEASE_PATHS.includes('vendor/ureq'))
  assert.ok(RELEASE_PATHS.includes('vendor/ureq/src'))
  assert.ok(REQUIRED_RELEASE_FILES.includes('vendor/ureq/Cargo.toml'))
  assert.ok(REQUIRED_RELEASE_FILES.includes('vendor/ureq/src/lib.rs'))
  assert.ok(RELEASE_PATHS.includes('cockpit/src'))
  assert.ok(RELEASE_PATHS.includes('cockpit/docs/COMPETITION_RUNNER.md'))
  assert.ok(RELEASE_PATHS.includes('SECURITY.md'))
  assert.ok(RELEASE_PATHS.includes('LICENSE'))
  assert.ok(RELEASE_PATHS.includes('scripts/check/check-legacy-terminal-boundary.sh'))
  assert.ok(RELEASE_PATHS.includes('release/supply-chain-policy.json'))
  assert.ok(RELEASE_PATHS.includes('release/Dockerfile.clean-builder'))
  assert.ok(RELEASE_PATHS.includes('docs/release-evidence.md'))
  assert.ok(REQUIRED_RELEASE_FILES.includes('cockpit/src/main.rs'))
  assert.ok(REQUIRED_RELEASE_FILES.includes('SECURITY.md'))
  assert.ok(REQUIRED_RELEASE_FILES.includes('LICENSE'))
  assert.ok(REQUIRED_RELEASE_FILES.includes('release/supply-chain-policy.json'))
  assert.ok(REQUIRED_RELEASE_FILES.includes('release/Dockerfile.clean-builder'))
  assert.ok(REQUIRED_RELEASE_FILES.includes('docs/release-evidence.md'))
  assert.ok(REQUIRED_RELEASE_FILES.includes('scripts/check/check-legacy-terminal-boundary.sh'))
  assert.equal(
    new Set(REQUIRED_COCKPIT_EMBEDDED_FILES).size,
    REQUIRED_COCKPIT_EMBEDDED_FILES.length,
  )

  const node = inspectNodeLock(ROOT)
  assert.equal(node.lockfile_version, 3)
  assert.equal(node.registry_package_count, 79)

  const rust = inspectCargoGraph(ROOT)
  assert.equal(rust.metadata_target, 'x86_64-unknown-linux-gnu')
  assert.equal(rust.toolchain.pinned_version, '1.95.0')
  assert.deepEqual(rust.toolchain.pinned_components, ['clippy', 'rustfmt'])
  assert.ok(rust.resolved_package_count > 300)
  assert.deepEqual(rust.license_files, [])
  assert.deepEqual(
    rust.prebuilt_artifacts.map((row) => `${row.name}@${row.crate_version}`),
    ['rusty_v8@150.0.0'],
  )
  assert.match(rust.clean_build_container_images[0].reference, /^sha256:[0-9a-f]{64}$/u)
  assert.equal(rust.advisory_scanners[0].version, '2.3.8')
  assert.deepEqual(
    rust.advisory_policy.lockfiles.map((row) => row.path),
    ['cockpit/Cargo.lock', 'cockpit/portal-renderer/Cargo.lock', 'package-lock.json'],
  )
  assert.deepEqual(
    rust.advisory_policy.lockfiles.map((row) => row.package_count),
    [370, 121, 79],
  )
  assert.equal(rust.advisory_policy.python.status, 'not-applicable')
  assert.deepEqual(
    rust.advisory_policy.exceptions.map((row) => row.package),
    ['bincode', 'paste', 'rustls-pemfile'],
  )
})

test('vendored dotmax keeps the unused imageproc branch out of the active graph', () => {
  const manifest = readFileSync(join(ROOT, 'vendor/dotmax/Cargo.toml'), 'utf8')
  assert.doesNotMatch(manifest, /^imageproc\s*=/mu)
  const tree = command(ROOT, 'cargo', [
    'tree',
    '--manifest-path',
    'cockpit/Cargo.toml',
    '--prefix',
    'none',
  ])
  for (const packageName of [
    'imageproc',
    'conv',
    'custom_derive',
    'ab_glyph',
    'owned_ttf_parser',
    'ttf-parser',
  ]) {
    assert.doesNotMatch(tree, new RegExp(`^${packageName} v`, 'mu'))
  }
})
