import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, utimesSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'

async function until(predicate) {
  const deadline = Date.now() + 8000
  while (!predicate()) {
    assert.ok(Date.now() < deadline, 'background build did not reach the expected state')
    await new Promise(resolve => setTimeout(resolve, 20))
  }
}

function launchFixture(t, { binary = true, buildFails = false, buildGate = false, blaze = false, blazeMode = 'standalone' } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'angel-launch-build-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const home = join(root, 'home'), source = join(root, 'source')
  for (const dir of ['bin', 'scripts/check', 'scripts/runtime', 'cockpit/target/release']) mkdirSync(join(source, dir), { recursive: true })
  mkdirSync(join(home, '.local/bin'), { recursive: true })
  copyFileSync(fileURLToPath(new URL('../../bin/angelX', import.meta.url)), join(source, 'bin/angelX'))
  copyFileSync(fileURLToPath(new URL('../../scripts/check/angel-club-policy.sh', import.meta.url)), join(source, 'scripts/check/angel-club-policy.sh'))
  const helper = fileURLToPath(new URL('../../scripts/runtime/background-build.py', import.meta.url))
  if (existsSync(helper)) copyFileSync(helper, join(source, 'scripts/runtime/background-build.py'))
  writeFileSync(join(source, 'scripts/runtime/build-native-modules.py'), 'import pathlib,sys\npathlib.Path(sys.argv[1], "modules-built").write_text("built")\n')
  const image = '#!/bin/sh\nprintf "cached-image\\n"\n'
  if (binary) {
    writeFileSync(join(source, 'cockpit/target/release/angel'), image)
    chmodSync(join(source, 'cockpit/target/release/angel'), 0o755)
  }
  writeFileSync(join(home, '.local/bin/rustc'), '#!/bin/sh\nprintf "host: x86_64-unknown-linux-gnu\\n"\n')
  chmodSync(join(home, '.local/bin/rustc'), 0o755)
  writeFileSync(join(home, '.local/bin/cargo'), `#!/bin/sh
if [ -n "$RUSTC_WORKSPACE_WRAPPER" ]; then exec "$RUSTC_WORKSPACE_WRAPPER" "$@"; fi
printf '%s\\n' "$*" >> "$HOME/builds"
if [ -f src/main.rs ]; then cat src/main.rs >> "$HOME/build-sources"; fi
${buildGate ? 'while [ ! -e "$HOME/allow-build" ]; do sleep 0.02; done' : ''}
${buildFails ? 'exit 7' : `profile=release
case "$*" in *'--profile fast'*) profile=fast ;; *'--release'*) ;; *) profile=debug ;; esac
mkdir -p "target/$profile"
printf '#!/bin/sh\\nprintf "built-image\\\\n"\\n' > "target/$profile/angel.new"
chmod +x "target/$profile/angel.new"
mv -f "target/$profile/angel.new" "target/$profile/angel"`}
`)
  chmodSync(join(home, '.local/bin/cargo'), 0o755)
  if (blaze) {
    const revision = 'b383b0bcf5da33816809b00a37e1c36a0e761efb'
    const toolDir = join(source, 'cockpit/target/tools/blaze', revision)
    mkdirSync(toolDir, { recursive: true })
    writeFileSync(join(source, 'cockpit/target/tools/blaze/selected'), revision + '\n')
    writeFileSync(join(toolDir, 'blaze'), `#!/bin/sh
if [ "$1" = toolchain ]; then printf '%s\n' "$HOME/.local"; exit 0; fi
printf '%s\n' "$*" >> "$HOME/blaze-builds"
printf '%s\n' "$CARGO_TARGET_DIR" >> "$HOME/blaze-targets"
${buildFails ? 'exit 7' : `profile=debug
case "$*" in *'--profile fast'*) profile=fast ;; *'--release'*) profile=release ;; esac
mkdir -p "$CARGO_TARGET_DIR/$profile"
# Blaze may patch its previous executable in place, unlike Cargo's rename.
printf '#!/bin/sh\nprintf "blaze-image\\n"\n' > "$CARGO_TARGET_DIR/$profile/angel"
chmod +x "$CARGO_TARGET_DIR/$profile/angel"`}
`)
    chmodSync(join(toolDir, 'blaze'), 0o755)
    writeFileSync(join(toolDir, 'ra-jit-rustc'), `#!/bin/sh
[ "$RA_JIT_SCRIPTS_BY_RUSTC" = 1 ] || exit 9
# The daemon reloads build scripts too; this supported switch makes those
# loads Rust-backed rather than hitting clang-sys's unsupported MIR.
[ "$RA_JIT_PROC_MACRO_SERVER" = 0 ] || { echo "daemon compatibility missing" >&2; exit 9; }
[ "$RUSTC" = "$HOME/.local/bin/rustc" ] || exit 9
exec "${toolDir}/blaze" "$@"
`)
    chmodSync(join(toolDir, 'ra-jit-rustc'), 0o755)
  }
  return {
    home, source,
    run(args = [], env = {}) {
      return spawnSync('bash', [join(source, 'bin/angelX'), ...args], {
        cwd: root, encoding: 'utf8', timeout: 10000,
        env: { HOME: home, PATH: '/usr/bin:/bin', LANG: 'C', ANGEL_GLASS: '0', ANGEL_BACKGROUND_BUILD: '0', ANGEL_HOT_SYSTEMS: '0', ...(blaze ? { ANGEL_BLAZE_MODE: blazeMode } : {}), ...env },
      })
    },
    builds() { return existsSync(join(home, 'builds')) ? readFileSync(join(home, 'builds'), 'utf8').trim().split('\n') : [] },
    status() {
      const path = join(source, 'cockpit/target/background-build.json')
      return existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : null
    },
    release() { writeFileSync(join(home, 'allow-build'), '') },
  }
}

test('plain angelX and inspection both check current source before launching the same application', (t) => {
  const fixture = launchFixture(t)
  const source = join(fixture.source, 'cockpit/src/main.rs')
  mkdirSync(join(fixture.source, 'cockpit/src'), { recursive: true })
  for (const [i, args] of [[], ['--version'], []].entries()) {
    writeFileSync(source, `edit-${i}\n`)
    const result = fixture.run(args)
    assert.equal(result.status, 0, result.stderr)
    assert.equal(result.stdout.trim(), 'built-image', 'must not run the previous completed image')
  }
  assert.equal(fixture.builds().length, 3)
  assert.equal(readFileSync(join(fixture.home, 'build-sources'), 'utf8'), 'edit-0\nedit-1\nedit-2\n')
  assert.ok(fixture.builds().every(command => command === fixture.builds()[0]), 'inspection must not switch to a smaller feature build')
  assert.ok(!fixture.builds()[0].includes('--no-default-features'))
  assert.equal(fixture.status(), null, 'launch is not deferred to a background worker')
})

test('ordinary launch with broken current source fails instead of opening a stale cockpit', (t) => {
  const fixture = launchFixture(t, { buildFails: true })
  for (const args of [[], ['--version']]) {
    const result = fixture.run(args, { ANGEL_BACKGROUND_BUILD: '1' })
    assert.equal(result.status, 1, result.stderr)
    assert.equal(result.stdout, '')
    assert.match(result.stderr, /build failed; refusing to run a stale binary/)
  }
  assert.equal(fixture.builds().length, 2)
  assert.equal(fixture.status(), null)
})

test('missing and existing images both go through the source freshness check', (t) => {
  const fixture = launchFixture(t, { binary: false })
  for (let i = 0; i < 2; ++i) {
    const result = fixture.run(['--version'])
    assert.equal(result.status, 0, result.stderr)
    assert.equal(result.stdout.trim(), 'built-image')
  }
  assert.equal(fixture.builds().length, 2)
})

test('explicit build is build-only and respects the fast profile', (t) => {
  const fixture = launchFixture(t)
  const result = fixture.run(['build'], { ANGEL_FAST: '1' })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout, '')
  assert.match(fixture.builds()[0], /^build --locked --profile fast/)
  assert.ok(existsSync(join(fixture.source, 'cockpit/target/fast/angel')))
  assert.ok(!existsSync(join(fixture.source, 'cockpit/target/fast/run')))
})

test('source builds default to incremental fast; release optimization is explicit', (t) => {
  const fixture = launchFixture(t)
  for (const [args, expected] of [[['build'], '--profile fast'], [['build', '--release'], '--release']]) {
    const result = fixture.run(args)
    assert.equal(result.status, 0, result.stderr)
    assert.equal(result.stdout, '')
    assert.ok(fixture.builds().at(-1).includes(expected), fixture.builds().at(-1))
  }
})

test('normal launches do not run mise hooks; PATH refresh is explicit', (t) => {
  const fixture = launchFixture(t)
  const mise = join(fixture.home, '.local/bin/mise')
  writeFileSync(mise, '#!/bin/sh\ntouch "$HOME/mise-called"\nprintf "{\\"PATH\\":\\"%s\\"}\\n" "$PATH"\n')
  chmodSync(mise, 0o755)
  const normal = fixture.run(['--version'])
  assert.equal(normal.status, 0, normal.stderr)
  assert.ok(!existsSync(join(fixture.home, 'mise-called')))
  const explicit = fixture.run(['--version'], { ANGEL_RESOLVE_MISE: '1' })
  assert.equal(explicit.status, 0, explicit.stderr)
  assert.ok(existsSync(join(fixture.home, 'mise-called')))
})

test('launcher logging uses the builtin clock without external date on modern Bash', (t) => {
  const version = spawnSync('bash', ['-c', 'printf "%s.%s" "${BASH_VERSINFO[0]}" "${BASH_VERSINFO[1]}"'], { encoding: 'utf8' })
  assert.equal(version.status, 0, version.stderr)
  const [major, minor] = version.stdout.split('.').map(Number)
  if (major < 4 || (major === 4 && minor < 2)) return t.skip('builtin clock requires Bash >=4.2')
  for (const mode of ['inspection', 'interactive', 'build-only', 'no-build', 'failed-build']) {
    const fixture = launchFixture(t, { buildFails: mode === 'failed-build' })
    writeFileSync(join(fixture.home, '.local/bin/date'), '#!/bin/sh\nprintf "date\\n" >> "$HOME/date-calls"\nexec /bin/date "$@"\n', { mode: 0o755 })
    const runner = join(fixture.home, 'runner.env')
    writeFileSync(runner, 'export TZ=Pacific/Honolulu\n')
    const start = Date.now()
    const args = mode === 'build-only' ? ['build'] : mode === 'interactive' ? [] : ['--version']
    const result = fixture.run(args, { TZ: 'UTC0', ...(mode === 'inspection' ? { ANGEL_RUNNER_ENV_FILE: runner } : {}), ...(mode === 'no-build' ? { ANGEL_NO_BUILD: '1' } : {}) })
    const end = Date.now()
    assert.equal(result.status, mode === 'failed-build' ? 1 : 0, result.stderr)
    assert.equal(result.stdout, ['failed-build', 'build-only'].includes(mode) ? '' : mode === 'no-build' ? 'cached-image\n' : 'built-image\n')
    assert.ok(!existsSync(join(fixture.home, 'date-calls')), `${mode} spawned external date`)
    const log = readFileSync(join(fixture.home, '.angelX/launch.log'), 'utf8')
    assert.match(log, /^=== \d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} launch  tty=/)
    assert.match(log, new RegExp(`\\d{2}:\\d{2}:\\d{2}   EXITED rc=${result.status}\\n$`))
    const stamp = log.match(/^=== (\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})/)[1]
    assert.ok(Date.parse(stamp.replace(' ', 'T') + 'Z') >= start - 1100)
    assert.ok(Date.parse(stamp.replace(' ', 'T') + 'Z') <= end)
    if (mode !== 'no-build') {
      const cargo = readFileSync(join(fixture.home, '.angelX/cargo.log'), 'utf8')
      assert.match(cargo, /^=== \d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} .*cargo --locked/)
      // Trusted runner configuration changes TZ after the first log entry.
      const offset = mode === 'inspection' ? '-10:00' : 'Z'
      const cargoStamp = cargo.match(/^=== (\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})/)[1]
      const parsed = Date.parse(cargoStamp.replace(' ', 'T') + offset)
      assert.ok(parsed >= start - 1100 && parsed <= end, `${mode}: stale timezone ${cargoStamp}`)
    }
    if (mode === 'interactive') assert.match(readFileSync(join(fixture.home, '.angelX/cockpit.err'), 'utf8'), /^=== \d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} cockpit stderr ===\n$/)
  }
})

test('launcher logging keeps external date fallback for pre-4.2 Bash', (t) => {
  const fixture = launchFixture(t)
  const launcher = join(fixture.source, 'bin/angelX')
  const source = readFileSync(launcher, 'utf8')
  // Force only version selection in this private copy; no production override.
  const gate = 'if (( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 2) )); then'
  assert.equal(source.split(gate).length, 2, 'must find one version gate')
  writeFileSync(launcher, source.replace(gate, 'if false; then'))
  writeFileSync(join(fixture.home, '.local/bin/date'), '#!/bin/sh\nprintf "%s\\n" "$*" >> "$HOME/date-calls"\nexec /bin/date "$@"\n', { mode: 0o755 })
  const result = fixture.run(['--version'], { TZ: 'UTC0' })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout, 'built-image\n')
  assert.deepEqual(readFileSync(join(fixture.home, 'date-calls'), 'utf8').trim().split('\n'), ['+%F %T', '+%T', '+%F %T', '+%T', '+%T', '+%T'])
  assert.match(readFileSync(join(fixture.home, '.angelX/launch.log'), 'utf8'), /\d{2}:\d{2}:\d{2}   EXITED rc=0\n$/)
})

test('failed fallback clock remains diagnostic-only and preserves exit and cleanup', (t) => {
  for (const mode of ['success', 'failed-build', 'cockpit-error', 'cockpit-signal']) {
    const fixture = launchFixture(t, { buildFails: mode === 'failed-build' })
    const launcher = join(fixture.source, 'bin/angelX')
    const gate = 'if (( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 2) )); then'
    writeFileSync(launcher, readFileSync(launcher, 'utf8').replace(gate, 'if false; then'))
    writeFileSync(join(fixture.home, '.local/bin/date'), '#!/bin/sh\nexit 17\n', { mode: 0o755 })
    const noBuild = mode.startsWith('cockpit-')
    if (noBuild) writeFileSync(join(fixture.source, 'cockpit/target/release/angel'), `#!/bin/sh
if [ "\${_ANGEL_LOG_STAMP+x}" = x ]; then echo "clock leaked" >&2; exit 99; fi
${mode === 'cockpit-signal' ? 'kill -TERM "$$"' : 'exit 23'}
`)
    const result = fixture.run(['--version'], { _ANGEL_LOG_STAMP: 'inherited-clock', ...(noBuild ? { ANGEL_NO_BUILD: '1' } : {}) })
    const expected = mode === 'failed-build' ? 1 : mode === 'cockpit-error' ? 23 : mode === 'cockpit-signal' ? 143 : 0
    assert.equal(result.status, expected, result.stderr)
    assert.equal(result.stdout, mode === 'success' ? 'built-image\n' : '')
    assert.ok(!result.stderr.includes('clock leaked'))
    const log = readFileSync(join(fixture.home, '.angelX/launch.log'), 'utf8')
    assert.ok(!log.includes('inherited-clock'))
    assert.match(log, new RegExp(`   EXITED rc=${expected}\\n$`))
    assert.deepEqual(existsSync(join(fixture.source, 'cockpit/target/release/run')) ? readdirSync(join(fixture.source, 'cockpit/target/release/run')) : [], [])
    assert.deepEqual(existsSync(join(fixture.source, 'cockpit/target/fast/run')) ? readdirSync(join(fixture.source, 'cockpit/target/fast/run')) : [], [])
  }
})

test('builtin logging does not export diagnostic state to the launched process', (t) => {
  const fixture = launchFixture(t)
  writeFileSync(join(fixture.source, 'cockpit/target/release/angel'), '#!/bin/sh\nif [ "${_ANGEL_LOG_STAMP+x}" = x ]; then exit 99; fi\nprintf "private-clock\\n"\n')
  const result = fixture.run(['--version'], { _ANGEL_LOG_STAMP: 'caller-export', ANGEL_NO_BUILD: '1' })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout, 'private-clock\n')
  assert.equal(result.stderr, '')
  assert.ok(!readFileSync(join(fixture.home, '.angelX/launch.log'), 'utf8').includes('caller-export'))
  assert.deepEqual(readdirSync(join(fixture.source, 'cockpit/target/release/run')), [])
})

test('configured RUSTUP_HOME reuses the pinned compiler without provisioning a new HOME', (t) => {
  const fixture = launchFixture(t)
  const rustupHome = join(fixture.home, 'shared toolchains')
  const bin = join(rustupHome, 'toolchains/fixture-pinned-x86_64-unknown-linux-gnu/bin')
  mkdirSync(bin, { recursive: true })
  writeFileSync(join(fixture.source, 'rust-toolchain.toml'), '[toolchain]\nchannel = "fixture-pinned"\n')
  const fakeBin = join(fixture.home, '.local/bin')
  const cargo = join(bin, 'cargo')
  writeFileSync(cargo, '#!/bin/sh\ntouch "$HOME/shared-cargo-used"\nexec "$HOME/.local/bin/cargo" "$@"\n')
  chmodSync(cargo, 0o755)
  writeFileSync(join(bin, 'rustc'), '#!/bin/sh\nprintf "host: x86_64-unknown-linux-gnu\\n"\n')
  chmodSync(join(bin, 'rustc'), 0o755)
  writeFileSync(join(fakeBin, 'rustc'), '#!/bin/sh\necho "unexpected rustc shim" >&2\nexit 94\n')
  chmodSync(join(fakeBin, 'rustc'), 0o755)
  writeFileSync(join(fakeBin, 'rustup'), '#!/bin/sh\necho "unexpected rustup provisioning" >&2\nexit 93\n')
  chmodSync(join(fakeBin, 'rustup'), 0o755)
  const result = fixture.run(['--version'], { RUSTUP_HOME: rustupHome })
  assert.equal(result.status, 0, result.stderr)
  assert.ok(existsSync(join(fixture.home, 'shared-cargo-used')))
  assert.doesNotMatch(result.stderr, /unexpected (rustc shim|rustup provisioning)/)
})

test('pinned compiler metadata is probed once per launch, never persisted between launches', (t) => {
  const fixture = launchFixture(t)
  const rustupHome = join(fixture.home, 'shared toolchains')
  const bin = join(rustupHome, 'toolchains/fixture-pinned-x86_64-unknown-linux-gnu/bin')
  mkdirSync(bin, { recursive: true })
  writeFileSync(join(fixture.source, 'rust-toolchain.toml'), '[toolchain]\nchannel = "fixture-pinned"\n')
  writeFileSync(join(bin, 'cargo'), '#!/bin/sh\nprintf "%s" "$ANGEL_BUILD_RUSTC" > "$HOME/compiler-metadata"\nexec "$HOME/.local/bin/cargo" "$@"\n', { mode: 0o755 })
  for (let i = 0; i < 2; ++i) {
    const info = `rustc fixture-${i}\nbinary: rustc\nhost: x86_64-unknown-linux-gnu\nrelease: fixture-${i}\nLLVM version: fixture`
    writeFileSync(join(bin, 'rustc'), `#!/bin/sh\nprintf 'probe\\n' >> "$HOME/compiler-probes"\nprintf '%s\\n' '${info}'\n`, { mode: 0o755 })
    const result = fixture.run(['--version'], { RUSTUP_HOME: rustupHome, ANGEL_BUILD_RUSTC: 'stale inherited metadata' })
    assert.equal(result.status, 0, result.stderr)
    assert.equal(result.stdout.trim(), 'built-image')
    assert.equal(readFileSync(join(fixture.home, 'compiler-metadata'), 'utf8'), info)
    assert.equal(readFileSync(join(fixture.home, 'compiler-probes'), 'utf8'), 'probe\n'.repeat(i + 1))
    assert.equal(fixture.builds().length, i + 1, 'every launch must still ask Cargo about freshness')
  }
})

test('host discovery never supplies metadata for a different selected compiler', (t) => {
  const fixture = launchFixture(t)
  const rustupHome = join(fixture.home, 'shared toolchains')
  const discovered = join(rustupHome, 'toolchains/fixture-pinned-aarch64-unknown-linux-gnu/bin')
  const selected = join(rustupHome, 'toolchains/fixture-pinned-x86_64-unknown-linux-gnu/bin')
  for (const bin of [discovered, selected]) mkdirSync(bin, { recursive: true })
  writeFileSync(join(fixture.source, 'rust-toolchain.toml'), '[toolchain]\nchannel = "fixture-pinned"\n')
  // The first installed compiler supplies the host used to select a different
  // sibling Cargo. Equal host triples do not make their full metadata equal.
  for (const [bin, name] of [[discovered, 'discovered'], [selected, 'selected']]) {
    writeFileSync(join(bin, 'rustc'), `#!/bin/sh\nprintf '${name}\\n' >> "$HOME/compiler-probes"\nprintf 'rustc ${name}\\nhost: x86_64-unknown-linux-gnu\\n'\n`, { mode: 0o755 })
  }
  writeFileSync(join(selected, 'cargo'), '#!/bin/sh\nprintf "%s" "$ANGEL_BUILD_RUSTC" > "$HOME/compiler-metadata"\nexec "$HOME/.local/bin/cargo" "$@"\n', { mode: 0o755 })
  const result = fixture.run(['--version'], { RUSTUP_HOME: rustupHome })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(readFileSync(join(fixture.home, 'compiler-metadata'), 'utf8'), 'rustc selected\nhost: x86_64-unknown-linux-gnu')
  assert.equal(readFileSync(join(fixture.home, 'compiler-probes'), 'utf8'), 'discovered\nselected\n')
  assert.equal(fixture.builds().length, 1)
})

test('failed PATH compiler discovery preserves installed Cargo fallback', (t) => {
  const fixture = launchFixture(t)
  const rustupHome = join(fixture.home, 'shared toolchains')
  const bin = join(rustupHome, 'toolchains/1.95.0-x86_64-unknown-linux-gnu/bin')
  mkdirSync(bin, { recursive: true })
  writeFileSync(join(fixture.source, 'rust-toolchain.toml'), '[toolchain]\nchannel = "fixture-pinned"\n')
  writeFileSync(join(bin, 'cargo'), '#!/bin/sh\nprintf "%s" "$ANGEL_BUILD_RUSTC" > "$HOME/compiler-metadata"\nexec "$HOME/.local/bin/cargo" "$@"\n', { mode: 0o755 })
  writeFileSync(join(bin, 'rustc'), '#!/bin/sh\nprintf "rustc selected\\nhost: x86_64-unknown-linux-gnu\\n"\n', { mode: 0o755 })
  for (const output of ['', 'host: x86_64-unknown-linux-gnu']) {
    writeFileSync(join(fixture.home, '.local/bin/rustc'), `#!/bin/sh\nprintf '%s' '${output}'\nexit 42\n`, { mode: 0o755 })
    const result = fixture.run(['--version'], { RUSTUP_HOME: rustupHome })
    assert.equal(result.status, 0, result.stderr)
    assert.equal(result.stdout.trim(), 'built-image')
    assert.equal(readFileSync(join(fixture.home, 'compiler-metadata'), 'utf8'), 'rustc selected\nhost: x86_64-unknown-linux-gnu')
  }
  assert.equal(fixture.builds().length, 2)
})

test('provisioning invalidates compiler metadata even when its path stays the same', (t) => {
  const fixture = launchFixture(t)
  const commands = join(fixture.home, '.local/bin')
  const rustupHome = join(fixture.home, 'shared toolchains')
  const bin = join(rustupHome, 'toolchains/fixture-pinned-x86_64-unknown-linux-gnu/bin')
  mkdirSync(bin, { recursive: true })
  copyFileSync(join(commands, 'cargo'), join(commands, 'fixture-cargo'))
  rmSync(join(commands, 'cargo'))
  writeFileSync(join(fixture.source, 'rust-toolchain.toml'), '[toolchain]\nchannel = "fixture-pinned"\n')
  writeFileSync(join(bin, 'rustc'), '#!/bin/sh\nprintf "rustc before-repair\\nhost: x86_64-unknown-linux-gnu\\n"\n', { mode: 0o755 })
  writeFileSync(join(bin, 'pending-cargo'), '#!/bin/sh\nprintf "%s" "$ANGEL_BUILD_RUSTC" > "$HOME/compiler-metadata"\nexec "$HOME/.local/bin/fixture-cargo" "$@"\n', { mode: 0o755 })
  writeFileSync(join(bin, 'pending-rustc'), '#!/bin/sh\nprintf "rustc after-repair\\nhost: x86_64-unknown-linux-gnu\\n"\n', { mode: 0o755 })
  writeFileSync(join(commands, 'rustup'), `#!/bin/sh
bin="$RUSTUP_HOME/toolchains/fixture-pinned-x86_64-unknown-linux-gnu/bin"
case "$1" in
  toolchain) mv "$bin/pending-rustc" "$bin/rustc"; mv "$bin/pending-cargo" "$bin/cargo" ;;
  which) printf '%s\\n' "$bin/cargo" ;;
  *) exit 92 ;;
esac
`, { mode: 0o755 })
  const result = fixture.run(['--version'], { RUSTUP_HOME: rustupHome })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout.trim(), 'built-image')
  assert.equal(readFileSync(join(fixture.home, 'compiler-metadata'), 'utf8'), 'rustc after-repair\nhost: x86_64-unknown-linux-gnu')
  assert.equal(fixture.builds().length, 1)
})

test('fresh pinned Rust provisioning includes the browser target before the first Cargo build', (t) => {
  const fixture = launchFixture(t)
  const commands = join(fixture.home, '.local/bin')
  const cargo = join(commands, 'cargo')
  // A fresh Rust home has no selected compiler yet. The rustup shim is
  // modeled below; no real installation or network request is made.
  copyFileSync(cargo, join(commands, 'fixture-cargo'))
  rmSync(cargo)
  writeFileSync(join(fixture.source, 'rust-toolchain.toml'), '[toolchain]\nchannel = "fixture-pinned"\ntargets = ["wasm32-unknown-unknown"]\n')
  writeFileSync(join(commands, 'rustup'), `#!/bin/sh
printf '%s\n' "$*" >> "$HOME/rustup-calls"
case "$*" in
  'toolchain install fixture-pinned --profile minimal --target wasm32-unknown-unknown')
    touch "$HOME/wasm-installed" ;;
  'which cargo --toolchain fixture-pinned')
    printf '%s\n' "$HOME/.local/bin/fixture-cargo" ;;
  *) echo incomplete-toolchain-install >&2; exit 92 ;;
esac
`)
  chmodSync(join(commands, 'rustup'), 0o755)
  const result = fixture.run(['--version'])
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout.trim(), 'built-image')
  assert.ok(existsSync(join(fixture.home, 'wasm-installed')))
  assert.equal(fixture.builds().length, 1)
  assert.equal(readFileSync(join(fixture.home, 'rustup-calls'), 'utf8'),
    'toolchain install fixture-pinned --profile minimal --target wasm32-unknown-unknown\nwhich cargo --toolchain fixture-pinned\n')
})

test('machine commands skip legacy process-table scans used only for cockpit log rotation', (t) => {
  const fixture = launchFixture(t)
  const marker = join(fixture.home, 'pgrep-called')
  const probe = join(fixture.home, '.local/bin/pgrep')
  writeFileSync(probe, `#!/bin/sh\ntouch '${marker}'\nexit 1\n`)
  chmodSync(probe, 0o755)
  const result = fixture.run(['--version'])
  assert.equal(result.status, 0, result.stderr)
  assert.ok(!existsSync(marker), 'machine startup must not scan unrelated processes')
})

test('native module builds skip Cargo and never launch the host', (t) => {
  const fixture = launchFixture(t, { binary: false, buildFails: true })
  const result = fixture.run(['build', '--modules'])
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout, '')
  assert.ok(existsSync(join(fixture.source, 'modules-built')))
  assert.deepEqual(fixture.builds(), [])
  assert.equal(fixture.status(), null)
})

test('native module builds reject no-build and host-only build options', (t) => {
  const fixture = launchFixture(t)
  for (const args of [['build', '--modules', '--background'], ['build', '--modules', '--release'], ['build', '--modules', '--modules']]) {
    assert.equal(fixture.run(args).status, 2)
  }
  const result = fixture.run(['build', '--modules'], { ANGEL_NO_BUILD: '1' })
  assert.equal(result.status, 1, result.stderr)
  assert.match(result.stderr, /ANGEL_NO_BUILD=1 forbids builds/)
  assert.ok(!existsSync(join(fixture.source, 'modules-built')))
  assert.deepEqual(fixture.builds(), [])
})

test('native reload paths are opt-in and explicit operator module paths win', (t) => {
  const fixture = launchFixture(t)
  const image = join(fixture.source, 'cockpit/target/release/angel')
  writeFileSync(image, '#!/bin/sh\nprintf "%s\\n" "${ANGEL_SKILL_ROUTER_MODULE:-unset}" "${ANGEL_MOA_ROUTING_MODULE:-unset}"\n')
  assert.equal(fixture.run(['--version'], { ANGEL_NO_BUILD: '1' }).stdout.trim(), 'unset\nunset')
  const custom = join(fixture.home, 'custom.so')
  const result = fixture.run(['--version'], { ANGEL_NO_BUILD: '1', ANGEL_NATIVE_MODULES: '1', ANGEL_SKILL_ROUTER_MODULE: custom })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout.trim(), `${custom}\n${join(fixture.source, 'cockpit/target/native/moa_router.so')}`)
  assert.deepEqual(fixture.builds(), [])
})

test('explicit no-build can select an existing incremental image without compiling', (t) => {
  const fixture = launchFixture(t, { buildFails: true })
  utimesSync(join(fixture.source, 'cockpit/target/release/angel'), 1, 1)
  mkdirSync(join(fixture.source, 'cockpit/target/fast'), { recursive: true })
  writeFileSync(join(fixture.source, 'cockpit/target/fast/angel'), '#!/bin/sh\nprintf "fast-image\\n"\n', { mode: 0o755 })
  const result = fixture.run(['--version'], { ANGEL_NO_BUILD: '1' })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout.trim(), 'fast-image')
  assert.deepEqual(fixture.builds(), [])
})

test('explicit no-build can reuse completed images; default launch never falls back by image timestamp', (t) => {
  const fixture = launchFixture(t, { buildFails: true })
  const fast = join(fixture.source, 'cockpit/target/fast/angel')
  mkdirSync(join(fixture.source, 'cockpit/target/fast'), { recursive: true })
  writeFileSync(fast, '#!/bin/sh\nprintf "old-fast-image\\n"\n', { mode: 0o755 })
  utimesSync(fast, 1, 1)
  for (const [env, expected] of [[{}, 'cached-image'], [{ ANGEL_FAST: '1' }, 'old-fast-image']]) {
    const result = fixture.run(['--version'], { ANGEL_NO_BUILD: '1', ...env })
    assert.equal(result.status, 0, result.stderr)
    assert.equal(result.stdout.trim(), expected)
  }
  assert.deepEqual(fixture.builds(), [])
  const result = fixture.run([])
  assert.equal(result.status, 1, result.stderr)
  assert.equal(result.stdout, '')
  assert.equal(fixture.builds().length, 1)
})

test('prebuilt installs keep their release image and never compile automatically', (t) => {
  const fixture = launchFixture(t, { buildFails: true })
  writeFileSync(join(fixture.source, 'PREBUILT.json'), '{}')
  const result = fixture.run([], { ANGEL_BACKGROUND_BUILD: '1' })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout.trim(), 'cached-image')
  assert.deepEqual(fixture.builds(), [])
  assert.equal(fixture.status(), null)
})

test('explicit rebuild failure never launches a cached image', (t) => {
  const fixture = launchFixture(t, { buildFails: true })
  for (const [args, env] of [[['build'], {}], [['--version'], { ANGEL_NO_BUILD: '0' }]]) {
    const result = fixture.run(args, env)
    assert.equal(result.status, 1, result.stderr)
    assert.equal(result.stdout, '')
    assert.match(result.stderr, /build failed; refusing to run a stale binary/)
  }
  assert.equal(fixture.builds().length, 2)
})

test('no-build remains strict when the selected image is missing', (t) => {
  const fixture = launchFixture(t, { binary: false })
  const result = fixture.run(['--version'], { ANGEL_NO_BUILD: '1' })
  assert.equal(result.status, 1, result.stderr)
  assert.deepEqual(fixture.builds(), [])
})

test('no-build forbids foreground and background build commands even when an image exists', (t) => {
  const fixture = launchFixture(t)
  for (const args of [['build'], ['build', '--release'], ['build', '--background']]) {
    const result = fixture.run(args, { ANGEL_NO_BUILD: '1' })
    assert.equal(result.status, 1, result.stderr)
    assert.match(result.stderr, /ANGEL_NO_BUILD=1 forbids builds/)
  }
  assert.deepEqual(fixture.builds(), [])
  assert.equal(fixture.status(), null)
})

test('explicit release background build keeps its selected profile despite development defaults', async (t) => {
  const fixture = launchFixture(t)
  const result = fixture.run(['build', '--release', '--background'], { ANGEL_DEV: '1', ANGEL_FAST: '1' })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout, '')
  await until(() => fixture.status()?.state === 'succeeded')
  assert.equal(fixture.status().profile, 'release')
  assert.match(fixture.builds()[0], /--release/)
  assert.ok(!fixture.builds()[0].includes('--profile fast'))
})

test('background builds pin fast and debug even when checkout configuration changes the profile', async (t) => {
  const fixture = launchFixture(t)
  writeFileSync(join(fixture.source, '.angel.env'), 'export ANGEL_DEV=1 ANGEL_FAST=0\n')
  let result = fixture.run(['build', '--background', '--profile', 'fast'])
  assert.equal(result.status, 0, result.stderr)
  await until(() => fixture.status()?.state === 'succeeded' && fixture.status()?.profile === 'fast')
  assert.match(fixture.builds().at(-1), /--profile fast/)
  writeFileSync(join(fixture.source, '.angel.env'), 'export ANGEL_DEV=0 ANGEL_FAST=1\n')
  result = fixture.run(['build', '--background', '--profile', 'debug'])
  assert.equal(result.status, 0, result.stderr)
  await until(() => fixture.status()?.state === 'succeeded' && fixture.status()?.profile === 'debug')
  assert.ok(!fixture.builds().at(-1).includes('--profile'))
  assert.ok(!fixture.builds().at(-1).includes('--release'))
})

test('build rejects trailing arguments before configuration or log effects', (t) => {
  const fixture = launchFixture(t)
  const result = fixture.run(['build', '--ask', 'do not run'])
  assert.equal(result.status, 2, result.stderr)
  assert.deepEqual(fixture.builds(), [])
  assert.ok(!existsSync(join(fixture.home, '.angelX')))
})

test('explicit no-build may open the cached image during a separately requested background build', async (t) => {
  const fixture = launchFixture(t, { buildGate: true })
  try {
    assert.equal(fixture.run(['build', '--background']).status, 0)
    for (let i = 0; i < 2; ++i) {
      const start = Date.now()
      const result = fixture.run([], { ANGEL_NO_BUILD: '1' })
      assert.equal(result.status, 0, result.stderr)
      assert.equal(result.stdout.trim(), 'cached-image')
      assert.ok(Date.now() - start < 2000, 'launch waited for the gated compiler')
      await until(() => fixture.builds().length === 1)
    }
    assert.equal(fixture.builds().length, 1)
    assert.equal(fixture.status()?.state, 'running')
  } finally {
    fixture.release()
  }
  await until(() => fixture.status()?.state === 'succeeded')
  assert.equal(fixture.builds().length, 1, 'ordinary opens must not queue redundant builds')
  const result = fixture.run(['--version'], { ANGEL_NO_BUILD: '1' })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout.trim(), 'built-image')
})

test('background build failure is logged without stopping launch or replacing the working image', async (t) => {
  const fixture = launchFixture(t, { buildFails: true })
  const result = fixture.run(['build', '--background'])
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout, '')
  await until(() => fixture.status()?.state === 'failed')
  assert.equal(fixture.status().exit_code, 1)
  assert.match(readFileSync(fixture.status().log, 'utf8'), /build failed; refusing to run a stale binary/)
  assert.equal(fixture.run(['--version'], { ANGEL_NO_BUILD: '1' }).stdout.trim(), 'cached-image')
})

test('a new request cannot reuse the previous build success while its worker starts', async (t) => {
  const fixture = launchFixture(t, { buildGate: true })
  try {
    assert.equal(fixture.run(['build', '--background']).status, 0)
    await until(() => fixture.status()?.state === 'running')
    fixture.release()
    await until(() => fixture.status()?.state === 'succeeded')
    rmSync(join(fixture.home, 'allow-build'))
    assert.equal(fixture.run(['build', '--background']).status, 0)
    assert.ok(['queued', 'running'].includes(fixture.status()?.state), 'stale success is not a new receipt')
    await until(() => fixture.builds().length === 2)
  } finally {
    fixture.release()
  }
  await until(() => fixture.status()?.state === 'succeeded')
})

test('explicit requests during compilation coalesce into one build of the later edits', async (t) => {
  const fixture = launchFixture(t, { buildGate: true })
  const source = join(fixture.source, 'cockpit/src/main.rs')
  mkdirSync(join(fixture.source, 'cockpit/src'), { recursive: true })
  writeFileSync(source, 'before-edit\n')
  try {
    assert.equal(fixture.run(['build', '--background']).status, 0)
    await until(() => existsSync(join(fixture.home, 'build-sources'))
      && readFileSync(join(fixture.home, 'build-sources'), 'utf8') === 'before-edit\n')
    writeFileSync(source, 'after-edit\n')
    for (let i = 0; i < 3; ++i) {
      const start = Date.now()
      const result = fixture.run(['build', '--background'])
      assert.equal(result.status, 0, result.stderr)
      assert.ok(Date.now() - start < 2000, 'request waited for the gated compiler')
    }
    const launch = fixture.run([], { ANGEL_NO_BUILD: '1' })
    assert.equal(launch.status, 0, launch.stderr)
    assert.equal(launch.stdout.trim(), 'cached-image')
    assert.equal(fixture.builds().length, 1, 'builds must never overlap')
  } finally {
    fixture.release()
  }
  await until(() => fixture.status()?.state === 'succeeded')
  assert.equal(fixture.builds().length, 2, 'later edits need one coalesced follow-up')
  assert.equal(readFileSync(join(fixture.home, 'build-sources'), 'utf8'), 'before-edit\nafter-edit\n')
})

test('a failed background build still services the latest queued profile request', async (t) => {
  const fixture = launchFixture(t, { buildGate: true, buildFails: true })
  try {
    assert.equal(fixture.run(['build', '--background', '--profile', 'fast']).status, 0)
    await until(() => fixture.builds().length === 1)
    for (const profile of ['release', 'debug']) {
      const result = fixture.run(['build', '--background', '--profile', profile])
      assert.equal(result.status, 0, result.stderr)
    }
    assert.equal(fixture.builds().length, 1)
  } finally {
    fixture.release()
  }
  await until(() => fixture.status()?.state === 'failed')
  assert.equal(fixture.builds().length, 2)
  assert.equal(fixture.status().profile, 'debug')
  assert.ok(!fixture.builds().at(-1).includes('--release'))
  assert.ok(!fixture.builds().at(-1).includes('--profile fast'))
  assert.equal(fixture.run(['--version'], { ANGEL_NO_BUILD: '1' }).stdout.trim(), 'cached-image')
})

test('explicit background build works without an existing image and does not open a cockpit', async (t) => {
  const fixture = launchFixture(t, { binary: false, buildGate: true })
  try {
    const result = fixture.run(['build', '--background'])
    assert.equal(result.status, 0, result.stderr)
    assert.equal(result.stdout, '')
    await until(() => fixture.status()?.state === 'running')
    assert.ok(!existsSync(join(fixture.source, 'cockpit/target/release/run')))
  } finally {
    fixture.release()
  }
  await until(() => fixture.status()?.state === 'succeeded')
})

test('only explicit background build commands schedule a worker; normal inspection checks current source', (t) => {
  const fixture = launchFixture(t)
  for (const [args, env] of [[[], { ANGEL_NO_BUILD: '1' }], [['--version'], {}]]) {
    const result = fixture.run(args, { ANGEL_BACKGROUND_BUILD: '1', ...env })
    assert.equal(result.status, 0, result.stderr)
  }
  assert.equal(fixture.builds().length, 1)
  assert.equal(fixture.status(), null)
})

// Exercise the real launch path with a compiler stub and profile probes.
// No Rust build or user configuration is needed.
function buildFixture(t, { prebuilt = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'angel-build-profile-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const home = join(root, 'home'), source = join(root, 'source')
  for (const path of [join(home, '.local/bin'), join(source, 'bin'), join(source, 'scripts/check'), join(source, 'cockpit')]) {
    mkdirSync(path, { recursive: true })
  }
  copyFileSync(fileURLToPath(new URL('../../bin/angelX', import.meta.url)), join(source, 'bin/angelX'))
  copyFileSync(fileURLToPath(new URL('../../scripts/check/angel-club-policy.sh', import.meta.url)), join(source, 'scripts/check/angel-club-policy.sh'))
  if (prebuilt) writeFileSync(join(source, 'PREBUILT.json'), '{"glass":true}\n')
  const probe = '#!/usr/bin/env python3\nimport json,os,sys\nprint(json.dumps({"profile":os.path.basename(os.path.dirname(os.path.dirname(sys.argv[0])))}))\n'
  for (const profile of ['fast', 'release', 'debug']) {
    const dir = join(source, 'cockpit/target', profile)
    mkdirSync(dir, { recursive: true })
    writeFileSync(join(dir, 'angel'), probe, { mode: 0o755 })
  }
  const log = join(root, 'cargo.jsonl')
  writeFileSync(join(home, '.local/bin/cargo'), `#!/usr/bin/env python3
import json,os,sys
with open(os.environ['FIXTURE_CARGO_LOG'], 'a') as log:
    record = {'args':sys.argv[1:],'profile':os.environ.get('ANGEL_BUILD_PROFILE')}
    if os.environ.get('FIXTURE_RECORD_SOURCE') == '1':
        record['source'] = os.environ.get('ANGEL_BUILD_SOURCE_SHA256')
    log.write(json.dumps(record)+'\\n')
sys.exit(1 if os.environ.get('FIXTURE_BUILD_FAIL') == '1' else 0)
`, { mode: 0o755 })
  writeFileSync(join(home, '.local/bin/rustc'), '#!/bin/sh\nprintf "rustc 1.95.0 (fixture)\\nhost: x86_64-unknown-linux-gnu\\n"\n', { mode: 0o755 })
  const launch = (extra = {}, args = ['--term']) => spawnSync('bash', [join(source, 'bin/angelX'), ...args], {
    cwd: root, encoding: 'utf8', timeout: 15000,
    env: { HOME: home, PATH: '/usr/bin:/bin', LANG: 'C', ANGEL_VIDEO: '0', ANGEL_HOT_SYSTEMS: '0', FIXTURE_CARGO_LOG: log, ...extra },
  })
  return { launch, source, builds: () => existsSync(log) ? readFileSync(log, 'utf8').trim().split('\n').map(line => JSON.parse(line)) : [] }
}

for (const [name, env, bound] of [
  ['fast source default', {}, false],
  ['debug source default', { ANGEL_DEV: '1' }, false],
  ['explicit source binding', { ANGEL_BIND_SOURCE: '1' }, true],
  ['release default', { ANGEL_FAST: '0' }, true],
]) {
  test(`${name} avoids unwanted native identity rebuilds`, (t) => {
    const fixture = buildFixture(t)
    const digest = 'a'.repeat(64)
    writeFileSync(join(fixture.source, 'scripts/check/cockpit-source-digest.sh'), `#!/bin/sh\nprintf '%s\\n' '${digest}'\n`)
    const result = fixture.launch({ FIXTURE_RECORD_SOURCE: '1', ...env })
    assert.equal(result.status, 0, result.stderr)
    assert.equal(fixture.builds()[0].source, bound ? digest : null)
  })
}

for (const [name, env, profile, flags] of [
  ['source default', {}, 'fast', ['--profile', 'fast']],
  ['explicit fast', { ANGEL_FAST: '1' }, 'fast', ['--profile', 'fast']],
  ['release opt-in', { ANGEL_FAST: '0' }, 'release', ['--release']],
  ['debug overrides fast', { ANGEL_DEV: '1', ANGEL_FAST: '1' }, 'debug', []],
]) {
  test(`launcher builds and runs the ${name} profile`, (t) => {
    const fixture = buildFixture(t)
    const result = fixture.launch(env)
    assert.equal(result.status, 0, result.stderr)
    assert.deepEqual(JSON.parse(result.stdout), { profile })
    assert.deepEqual(fixture.builds(), [{ args: ['build', '--locked', ...flags, '--no-default-features', '--features', 'glass,delve'], profile }])
  })
}

test('ANGEL_DELVE=0 builds the console without the Delve', (t) => {
  const fixture = buildFixture(t)
  const result = fixture.launch({ ANGEL_DELVE: '0' })
  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(fixture.builds(), [{ args: ['build', '--locked', '--profile', 'fast', '--no-default-features', '--features', 'glass'], profile: 'fast' }])
})

test('machine source launches also use fast and build the same features as the window', (t) => {
  const fixture = buildFixture(t)
  const result = fixture.launch({}, ['--build-info', '--json'])
  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(JSON.parse(result.stdout), { profile: 'fast' })
  assert.deepEqual(fixture.builds(), [{ args: ['build', '--locked', '--profile', 'fast', '--no-default-features', '--features', 'glass,delve'], profile: 'fast' }])
})

for (const [name, prebuilt, env, profile] of [
  ['source fast', false, { ANGEL_NO_BUILD: '1' }, 'fast'],
  ['source release', false, { ANGEL_NO_BUILD: '1', ANGEL_FAST: '0' }, 'release'],
  ['installed release', true, {}, 'release'],
]) {
  test(`launcher reopens ${name} without invoking Cargo`, (t) => {
    const fixture = buildFixture(t, { prebuilt })
    const result = fixture.launch(env)
    assert.equal(result.status, 0, result.stderr)
    assert.deepEqual(JSON.parse(result.stdout), { profile })
    assert.deepEqual(fixture.builds(), [])
  })
}

test('a failed fast build never launches the previous executable', (t) => {
  const fixture = buildFixture(t)
  const result = fixture.launch({ FIXTURE_BUILD_FAIL: '1' })
  assert.notEqual(result.status, 0)
  assert.equal(result.stdout, '')
  assert.match(result.stderr, /cockpit build failed; refusing to run a stale binary/)
  assert.equal(fixture.builds().length, 1)
})

test('CLI global authority wins interactive and runner env conflicts; native intent stays opaque', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-native-globals-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const home = join(root, 'home'), source = join(root, 'source')
  for (const dir of ['bin', 'scripts/check', 'cockpit/target/fast']) mkdirSync(join(source, dir), { recursive: true })
  mkdirSync(home)
  copyFileSync(fileURLToPath(new URL('../../bin/angelX', import.meta.url)), join(source, 'bin/angelX'))
  copyFileSync(fileURLToPath(new URL('../../scripts/check/angel-club-policy.sh', import.meta.url)), join(source, 'scripts/check/angel-club-policy.sh'))
  const probe = join(source, 'cockpit/target/fast/angel')
  writeFileSync(probe, `#!/usr/bin/env python3
import json,os,sys
print(json.dumps({'argv':sys.argv[1:],'yolo':os.environ.get('ANGEL_YOLO'),'comp':os.environ.get('ANGEL_COMP_MODE'),'turbo':os.environ.get('ANGEL_TURBO')}))
`)
  chmodSync(probe, 0o755)
  const config = 'ANGEL_YOLO=0\nANGEL_COMP_MODE=0\nANGEL_TURBO=0\n'
  writeFileSync(join(source, '.angel.env'), config)
  const runner = join(root, 'runner.env')
  writeFileSync(runner, config)
  for (const args of [[], ['--ask', '--doctor'], ['--labyrinth', '--workspace', root, 'plan']]) {
    for (const alias of ['--comp', '--lean', '--turbo', '--angelturbo']) {
      const result = spawnSync('bash', [join(source, 'bin/angelX'), '--yolo', alias, ...args], {
        cwd: root, encoding: 'utf8', timeout: 15000,
        env: { HOME: home, PATH: '/usr/bin:/bin', LANG: 'C', ANGEL_NO_BUILD: '1', ANGEL_VIDEO: '0', ANGEL_RUNNER_ENV_FILE: runner },
      })
      assert.equal(result.status, 0, result.stderr)
      assert.deepEqual(JSON.parse(result.stdout), { argv: args, yolo: '1', comp: '1', turbo: '1' })
    }
  }
  const args = ['--driver', 'openai', '--model', 'exact/slash-ID', '--effort', 'pi:high', '--prompt', '--doctor']
  const result = spawnSync('bash', [join(source, 'bin/angelX'), ...args], {
    cwd: root, encoding: 'utf8', timeout: 15000,
    env: { HOME: home, PATH: '/usr/bin:/bin', LANG: 'C', ANGEL_NO_BUILD: '1', ANGEL_VIDEO: '0' },
  })
  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(JSON.parse(result.stdout), { argv: args, yolo: '0', comp: '0', turbo: '0' })
})


test('auto keeps the qualified Cargo path even with Blaze installed', (t) => {
  const fixture = launchFixture(t, { blaze: true })
  const result = fixture.run(['--version'])
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout.trim(), 'built-image')
  assert.equal(fixture.builds().length, 1)
  assert.ok(!existsSync(join(fixture.home, 'blaze-builds')))
})

test('explicit Blaze builds with full features and an isolated target', (t) => {
  const fixture = launchFixture(t, { blaze: true })
  for (const args of [[], ['--version']]) {
    const result = fixture.run(args, { ANGEL_BUILD_BACKEND: 'blaze' })
    assert.equal(result.status, 0, result.stderr)
    assert.equal(result.stdout.trim(), 'blaze-image')
  }
  assert.deepEqual(fixture.builds(), [], 'must build through Blaze, not Cargo')
  const builds = readFileSync(join(fixture.home, 'blaze-builds'), 'utf8').trim().split('\n')
  assert.equal(builds.length, 2)
  assert.equal(builds[0], builds[1], 'inspection must use identical features')
  assert.match(builds[0], /^build --locked --profile fast/)
  assert.match(builds[0], /--features glass/)
  assert.ok(!builds[0].includes('--no-default-features'))
  assert.equal(readFileSync(join(fixture.home, 'blaze-targets'), 'utf8').trim().split('\n')[0], join(fixture.source, 'cockpit/target/blaze'))
})

test('explicit Cargo bypasses installed Blaze; explicit Blaze requires the tool', (t) => {
  const fixture = launchFixture(t, { blaze: true })
  const cargo = fixture.run(['--version'], { ANGEL_BUILD_BACKEND: 'cargo' })
  assert.equal(cargo.status, 0, cargo.stderr)
  assert.equal(cargo.stdout.trim(), 'built-image')
  assert.ok(!existsSync(join(fixture.home, 'blaze-builds')))
  const missing = launchFixture(t)
  const result = missing.run(['--version'], { ANGEL_BUILD_BACKEND: 'blaze' })
  assert.equal(result.status, 1)
  assert.match(result.stderr, /Blaze.*not installed/)
  assert.equal(result.stdout, '')
  assert.deepEqual(missing.builds(), [])
})

test('Blaze failure never falls back to Cargo or a cached image', (t) => {
  const fixture = launchFixture(t, { blaze: true, buildFails: true })
  const result = fixture.run(['--version'], { ANGEL_BUILD_BACKEND: 'blaze' })
  assert.equal(result.status, 1)
  assert.equal(result.stdout, '')
  assert.match(result.stderr, /build failed; refusing to run a stale binary/)
  assert.deepEqual(fixture.builds(), [])
})

test('Blaze launch image is copied, not hardlinked to the in-place patch target', (t) => {
  const fixture = launchFixture(t, { blaze: true })
  const tool = join(fixture.source, 'cockpit/target/tools/blaze/b383b0bcf5da33816809b00a37e1c36a0e761efb/blaze')
  // Emit a program that changes the original target while the launch image runs.
  writeFileSync(tool, `#!/bin/sh
mkdir -p "$CARGO_TARGET_DIR/fast"
printf '#!/bin/sh\nprintf "#!/bin/sh\\necho changed\\n" > "$CARGO_TARGET_DIR/fast/angel"\nprintf "private-image-ok\\n"\n' > "$CARGO_TARGET_DIR/fast/angel"
chmod +x "$CARGO_TARGET_DIR/fast/angel"
`)
  const result = fixture.run(['--version'], { ANGEL_BUILD_BACKEND: 'blaze' })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout.trim(), 'private-image-ok')
})


test('invalid or missing project-local Blaze selection fails closed; no-build ignores it', (t) => {
  const fixture = launchFixture(t, { blaze: true })
  const selector = join(fixture.source, 'cockpit/target/tools/blaze/selected')
  for (const value of ['../../somewhere', 'a'.repeat(40)]) {
    writeFileSync(selector, value + '\n')
    const result = fixture.run(['--version'], { ANGEL_BUILD_BACKEND: 'blaze' })
    assert.equal(result.status, 1)
    assert.equal(result.stdout, '')
    assert.deepEqual(fixture.builds(), [])
  }
  const cached = fixture.run(['--version'], { ANGEL_NO_BUILD: '1' })
  assert.equal(cached.status, 0, cached.stderr)
  assert.equal(cached.stdout.trim(), 'cached-image')
})


test('explicit Blaze wrapper builds the full application with its own Rust ABI', (t) => {
  const fixture = launchFixture(t, { blaze: true, blazeMode: 'wrapper' })
  const result = fixture.run(['--version'], { ANGEL_BUILD_BACKEND: 'blaze' })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout.trim(), 'blaze-image')
  assert.match(result.stderr, /Blaze workspace-wrapper mode/)
  assert.deepEqual(fixture.builds(), [])
  const builds = readFileSync(join(fixture.home, 'blaze-builds'), 'utf8')
  assert.match(builds, /--profile fast --features glass/)
  assert.ok(!builds.includes('--no-default-features'))
})

test('short printable ASCII launch text avoids Python while preserving arguments', (t) => {
  const fixture = launchFixture(t)
  writeFileSync(join(fixture.home, '.local/bin/python3'), '#!/bin/sh\nprintf "called\\n" >> "$HOME/python-calls"\nexec /usr/bin/python3 "$@"\n', { mode: 0o755 })
  // Echo argv without another language runtime, keeping locale state observable.
  writeFileSync(join(fixture.source, 'cockpit/target/release/angel'), '#!/bin/sh\nprintf "locale=%s\\n" "$LC_ALL"\nprintf "<%s>\\n" "$@"\n')
  const env = { ANGEL_NO_BUILD: '1', LC_ALL: 'C.UTF-8', PATH: `${fixture.home}/.local/bin:/usr/bin:/bin` }
  for (const args of [
    ['--draft', '  hello  ', '--driver', 'practice', '--model', 'a/b:local', '--effort', 'high'],
    ['--resume', ' session id ', '--driver', 'practice'],
    ['--driver', 'x'.repeat(1024)],
  ]) {
    const result = fixture.run(args, env)
    assert.equal(result.status, 0, result.stderr)
    assert.equal(result.stdout, 'locale=C.UTF-8\n' + args.map(value => `<${value}>\n`).join(''))
  }
  assert.ok(!existsSync(join(fixture.home, 'python-calls')), 'common ASCII text must not spawn Python')
  for (const args of [['--draft', ' '], ['--driver', ' x'], ['--driver', 'x '], ['--draft', '']]) {
    const result = fixture.run(args, env)
    assert.equal(result.status, 2, result.stderr)
    assert.equal(result.stdout, '')
  }
  assert.ok(!existsSync(join(fixture.home, 'python-calls')), 'ASCII rejection must not spawn Python either')
})

test('non-ASCII and long launch text retains real Python validation and failure handling', (t) => {
  const fixture = launchFixture(t)
  const wrapper = join(fixture.home, '.local/bin/python3')
  writeFileSync(wrapper, '#!/bin/sh\nprintf "called\\n" >> "$HOME/python-calls"\nexec /usr/bin/python3 "$@"\n', { mode: 0o755 })
  const cases = [
    [['--draft', '日本語'], 0],
    [['--driver', '\u00a0x'], 2],
    [['--driver', '\u001cx'], 0], // Rust's explicit whitespace set is not Python str.strip().
    [['--draft', 'x'.repeat(1025)], 0],
    [['--driver', 'x'.repeat(1025)], 2],
    [['--draft', '\t\n'], 2],
  ]
  for (const [args, status] of cases) {
    const result = fixture.run(args, { ANGEL_NO_BUILD: '1', PATH: `${fixture.home}/.local/bin:/usr/bin:/bin` })
    assert.equal(result.status, status, result.stderr)
    assert.equal(result.stdout, status === 0 ? 'cached-image\n' : '')
  }
  assert.equal(readFileSync(join(fixture.home, 'python-calls'), 'utf8'), 'called\n'.repeat(cases.length))
  writeFileSync(wrapper, '#!/bin/sh\nexit 19\n')
  const failed = fixture.run(['--draft', '日本語'], { ANGEL_NO_BUILD: '1', PATH: `${fixture.home}/.local/bin:/usr/bin:/bin` })
  assert.equal(failed.status, 2, failed.stderr)
  assert.equal(failed.stdout, '')
})

test('launch text validation remains enforced when Python optimization is inherited', (t) => {
  const fixture = launchFixture(t)
  for (const level of ['1', '2']) {
    for (const [args, status] of [
      [['--draft', ' '], 2],
      [['--draft', ' '.repeat(1025)], 2],
      [['--draft', '\u3000'], 2],
      [['--draft', '日本語'], 0],
      [['--driver', '\u00a0x'], 2],
      [['--driver', 'x'.repeat(1025)], 2],
      [['--driver', '\u001cx'], 0],
    ]) {
      const result = fixture.run(args, { ANGEL_NO_BUILD: '1', PYTHONOPTIMIZE: level })
      assert.equal(result.status, status, result.stderr)
      assert.equal(result.stdout, status === 0 ? 'cached-image\n' : '')
    }
  }
})
