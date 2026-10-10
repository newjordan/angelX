import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'

test('CLI global authority wins interactive and runner env conflicts; native intent stays opaque', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'angel-native-globals-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const home = join(root, 'home'), source = join(root, 'source')
  for (const dir of ['bin', 'scripts/check', 'cockpit/target/release']) mkdirSync(join(source, dir), { recursive: true })
  mkdirSync(home)
  copyFileSync(fileURLToPath(new URL('../../bin/angelX', import.meta.url)), join(source, 'bin/angelX'))
  copyFileSync(fileURLToPath(new URL('../../scripts/check/angel-club-policy.sh', import.meta.url)), join(source, 'scripts/check/angel-club-policy.sh'))
  const probe = join(source, 'cockpit/target/release/angel')
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
        env: { HOME: home, PATH: '/usr/bin:/bin', LANG: 'C', ANGEL_NO_BUILD: '1', ANGEL_VIDEO: '0', ANGEL_WEBGPU_PORTAL: '0', ANGEL_RUNNER_ENV_FILE: runner },
      })
      assert.equal(result.status, 0, result.stderr)
      assert.deepEqual(JSON.parse(result.stdout), { argv: args, yolo: '1', comp: '1', turbo: '1' })
    }
  }
  const args = ['--driver', 'openai', '--model', 'exact/slash-ID', '--effort', 'pi:high', '--prompt', '--doctor']
  const result = spawnSync('bash', [join(source, 'bin/angelX'), ...args], {
    cwd: root, encoding: 'utf8', timeout: 15000,
    env: { HOME: home, PATH: '/usr/bin:/bin', LANG: 'C', ANGEL_NO_BUILD: '1', ANGEL_VIDEO: '0', ANGEL_WEBGPU_PORTAL: '0' },
  })
  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(JSON.parse(result.stdout), { argv: args, yolo: '0', comp: '0', turbo: '0' })
})
