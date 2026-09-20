#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { arch, platform, release } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { canonicalJson, sha256File } from './release-evidence.mjs'
import { installCandidate, PREFIX_INSTALL_SCHEMA } from './verify-release-install.mjs'

export function runRollbackVerification({ previous, next, previousSha256, nextSha256, receiptPath, worktree = process.cwd() }) {
  const receipt = { schema: 'angel0-prefix-rollback-verification/v1',
    timestamp_utc: new Date().toISOString(), host: { platform: platform(), arch: arch(), release: release() },
    status: 'fail', steps: [],
    coverage: { executable: 'bin/angel', operation: 'N -> N+1 -> N using retained N artifact',
      not_covered: ['bin/angel0 launcher and aliases/shims', 'angel-portal-renderer helper',
        'interactive terminal operation', 'operator state migration', 'package manager integration',
        'clean-host dependency provisioning', 'other supported hosts'],
      network_isolation: 'build-info only with empty credential environment; no network namespace claim' } }
  let scratch
  const output = resolve(receiptPath)
  function journal(step) {
    receipt.steps.push({ timestamp_utc: new Date().toISOString(), ...step })
    mkdirSync(dirname(output), { recursive: true })
    writeFileSync(output, canonicalJson(receipt))
  }
  try {
    if (platform() !== 'linux' || arch() !== 'x64') throw new Error('supported host is Linux x86_64')
    const candidates = [previous, next].map((path, index) => {
      const expected = [previousSha256, nextSha256][index]
      if (!/^[a-f0-9]{64}$/.test(expected || '')) throw new Error('explicit reviewed candidate sha256 required')
      const actual = sha256File(path)
      if (actual !== expected) throw new Error(`candidate ${index} differs from reviewed sha256`)
      return { path: resolve(path), sha256: actual, metadata: { schema: PREFIX_INSTALL_SCHEMA,
        release: { reviewed_binary_sha256: expected }, binary: { bytes: statSync(path).size,
          sha256: actual, mode: '0755', platform: 'linux-x86_64' } } }
    })
    if (candidates[0].sha256 === candidates[1].sha256) throw new Error('N and N+1 must be distinct artifacts')
    receipt.candidates = candidates.map(({ path, sha256 }) => ({ path, sha256 }))
    scratch = mkdtempSync(join(resolve(worktree), '.angel-rollback-'))
    const home = join(scratch, 'home')
    mkdirSync(home)
    function probe(binary) {
      const command = [binary, '--build-info', '--json']
      const result = spawnSync(binary, command.slice(1), { cwd: home,
        env: { HOME: home, PATH: '/usr/bin:/bin', LANG: 'C.UTF-8', ANGEL_NO_BUILD: '1' }, encoding: 'utf8' })
      if (result.error || result.status !== 0) throw new Error(`build-info probe failed: exit ${result.status}`)
      const info = JSON.parse(result.stdout)
      if (info?.schema !== 'angel-build-info/v1' || !/^[a-f0-9]{64}$/.test(info.cockpit_source_sha256 || ''))
        throw new Error('build-info lacks source identity')
      return { command, exit_code: result.status, build_info: info, sha256: sha256File(binary) }
    }
    const identities = candidates.map((c, index) => {
      const result = probe(c.path)
      journal({ operation: `source-${index === 0 ? 'N' : 'N+1'}`, ...result })
      return result.build_info
    })
    if (canonicalJson(identities[0]) === canonicalJson(identities[1])) throw new Error('build identities must differ')
    for (const [index, operation] of [[0, 'install-N'], [1, 'upgrade-N+1'], [0, 'rollback-N']]) {
      const candidate = candidates[index]
      journal({ operation, phase: 'started', expected_sha256: candidate.sha256 })
      const installed = installCandidate({ sandboxRoot: scratch, candidate: candidate.path, metadata: candidate.metadata })
      const result = probe(installed.layout.binary)
      if (result.sha256 !== candidate.sha256 || canonicalJson(result.build_info) !== canonicalJson(identities[index]))
        throw new Error(`${operation} installed identity mismatch`)
      journal({ operation, phase: 'verified', replaced: installed.replaced, ...result,
        metadata: JSON.parse(readFileSync(installed.layout.metadata, 'utf8')) })
    }
    receipt.status = 'pass'
  } catch (error) {
    receipt.error = error.message
    journal({ operation: 'failure', error: error.message })
  } finally {
    if (scratch) rmSync(scratch, { recursive: true, force: true })
    receipt.cleanup = { prefix_created: Boolean(scratch), removed: Boolean(scratch) && !existsSync(scratch) }
    journal({ operation: 'finished', status: receipt.status })
  }
  return receipt
}

if (import.meta.url === (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href)) {
  const args = process.argv.slice(2), options = {}
  const names = { '--previous': 'previous', '--next': 'next', '--previous-sha256': 'previousSha256',
    '--next-sha256': 'nextSha256', '--receipt': 'receiptPath' }
  try {
    for (let i = 0; i < args.length; i += 2) {
      if (!names[args[i]] || !args[i + 1]) throw new Error(`unknown/incomplete argument: ${args[i]}`)
      options[names[args[i]]] = args[i + 1]
    }
    const receipt = runRollbackVerification(options)
    console.log(`install rollback: ${receipt.status}${receipt.error ? `: ${receipt.error}` : ''}`)
    process.exitCode = receipt.status === 'pass' ? 0 : 1
  } catch (error) { console.error(error.message); process.exitCode = 1 }
}
