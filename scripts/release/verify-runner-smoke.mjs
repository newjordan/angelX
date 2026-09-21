#!/usr/bin/env node

import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, lstatSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

export const RUNNER_SMOKE_SCHEMA = 'angelX-runner-contract-smoke/v1'

const BUBBLEWRAP = '/usr/bin/bwrap'
const MAX_OUTPUT = 4 * 1024 * 1024
const TIMEOUT_MS = 60_000

function fail(message) {
  throw new Error(`runner contract smoke: ${message}`)
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex')
}

function canonicalValue(value) {
  if (Array.isArray(value)) return value.map(canonicalValue)
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, canonicalValue(value[key])]),
    )
  }
  return value
}

function assertSha256(value, label) {
  if (!/^[0-9a-f]{64}$/u.test(String(value || ''))) fail(`${label} is not a SHA-256`)
}

function exactKeys(value, expected, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) fail(`${label} is not an object`)
  const actual = Object.keys(value).sort()
  const wanted = [...expected].sort()
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    const missing = wanted.filter((field) => !actual.includes(field))
    const unexpected = actual.filter((field) => !wanted.includes(field))
    fail(
      `${label} fields differ from contract (missing: ${missing.join(', ') || 'none'}; unexpected: ${unexpected.join(', ') || 'none'})`,
    )
  }
}

function parseSingleJson(stdout, label) {
  const text = String(stdout).trim()
  if (!text || text.includes('\n')) fail(`${label} did not emit exactly one JSON line`)
  try {
    return JSON.parse(text)
  } catch {
    fail(`${label} did not emit valid JSON`)
  }
}

function systemMounts() {
  return ['/usr', '/bin', '/sbin', '/lib', '/lib64', '/etc'].flatMap((path) =>
    existsSync(path) ? ['--ro-bind', path, path] : [],
  )
}

function sandboxArgs(root, binary, args) {
  return [
    '--die-with-parent',
    '--new-session',
    '--unshare-user',
    '--unshare-pid',
    '--unshare-ipc',
    '--unshare-uts',
    '--unshare-net',
    '--cap-drop',
    'ALL',
    ...systemMounts(),
    '--proc',
    '/proc',
    '--dev',
    '/dev',
    '--tmpfs',
    '/tmp',
    '--dir',
    '/runner',
    '--ro-bind',
    binary,
    '/runner/angel',
    '--dir',
    '/smoke',
    '--bind',
    root,
    '/smoke',
    '--chdir',
    '/smoke/workspace',
    '--setenv',
    'HOME',
    '/smoke/home',
    '--setenv',
    'ANGEL_HARNESS_ROLLOUT_DIR',
    '/smoke/rollouts',
    '--setenv',
    'LANG',
    'C.UTF-8',
    '--setenv',
    'LC_ALL',
    'C.UTF-8',
    // Headless mode refuses the offline practice driver unless the caller opts
    // in explicitly; the smoke selects `--driver practice` on purpose.
    '--setenv',
    'ANGEL_PRACTICE',
    '1',
    '/runner/angel',
    ...args,
  ]
}

function execute(root, binary, args, { input = '', expectedStatus = 0 } = {}) {
  const result = spawnSync(BUBBLEWRAP, sandboxArgs(root, binary, args), {
    cwd: '/',
    env: { HOME: root, PATH: '/usr/bin:/bin', LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8' },
    input,
    encoding: 'utf8',
    maxBuffer: MAX_OUTPUT,
    timeout: TIMEOUT_MS,
    stdio: ['pipe', 'pipe', 'pipe'],
  })
  if (result.error) fail(`could not execute sandboxed runner: ${result.error.message}`)
  if (result.status !== expectedStatus) {
    fail(
      `runner exited ${result.status}, expected ${expectedStatus}: ${String(result.stderr).trim().slice(-2000)}`,
    )
  }
  return result
}

export function validateRunnerSmokeEnvelope(envelope, expected) {
  const runtime = envelope?.runtime
  const runtimeMaterial = {
    schema: 'angel-task-runtime/v1',
    runner_version: runtime?.runner_version,
    cockpit_source_sha256: runtime?.cockpit_source_sha256,
    prompt_sha256: expected.promptSha256,
    requested_driver: 'practice',
    requested_reasoning_effort: null,
    requested_task_pace: 'auto',
    task_pace: 'rapid',
    task_pace_source: 'default',
    max_hops: 2,
    deadline_secs: 30,
    tool_profile: 'essential',
    rollout_capture: 'local',
    rollout_required: true,
    unrestricted: false,
    verification_policy: 'external-only',
  }
  exactKeys(
    runtime,
    [
      'config_sha256',
      ...Object.entries(runtimeMaterial)
        .filter(([, value]) => value !== null)
        .map(([field]) => field),
    ],
    'task runtime config',
  )
  if (
    envelope?.version !== 1 ||
    envelope?.kind !== 'angel.task_result' ||
    envelope?.task_id !== expected.taskId ||
    envelope?.run_id !== expected.runId ||
    envelope?.workspace !== '/smoke/workspace' ||
    envelope?.status !== 'completed' ||
    envelope?.stop_reason !== 'answer' ||
    envelope?.interrupted !== false ||
    envelope?.deadline_reached !== false ||
    envelope?.max_hops_reached !== false ||
    !runtime ||
    Object.entries(runtimeMaterial).some(([field, value]) => (runtime[field] ?? null) !== value) ||
    typeof runtime.runner_version !== 'string' ||
    !runtime.runner_version ||
    (runtime.cockpit_source_sha256 !== 'unbound' &&
      !/^[0-9a-f]{64}$/u.test(String(runtime.cockpit_source_sha256 || '')))
  ) {
    fail('task result does not match the finite external-verifier contract')
  }
  assertSha256(envelope.runtime.config_sha256, 'runtime config digest')
  if (envelope.runtime.config_sha256 !== sha256(JSON.stringify(canonicalValue(runtimeMaterial)))) {
    fail('runtime config digest does not verify')
  }
  if (
    !/^rol-[0-9a-f]{1,16}-[0-9a-f]{1,16}-[0-9a-f]{1,16}$/u.test(String(envelope.rollout_id || ''))
  ) {
    fail('task result lacks a valid exact rollout ID')
  }
  if (typeof envelope.answer !== 'string' || envelope.answer.length === 0) {
    fail('practice task did not produce an answer')
  }
}

export function validateRunnerSmokeAudit(receipt, envelope, expected) {
  const binding = receipt?.task_binding
  if (
    receipt?.schema !== 'angel-harness-rollout-audit/v1' ||
    receipt?.rollout_id !== envelope.rollout_id ||
    receipt?.project?.schema !== 'angel-project-digest/v1' ||
    receipt?.capture?.mode !== 'local' ||
    receipt?.capture?.semantic_bodies !== true ||
    receipt?.capture?.media_bodies !== false ||
    receipt?.capture?.private_reasoning_captured !== false ||
    receipt?.capture?.provider_headers_captured !== false ||
    receipt?.status !== 'finalized' ||
    receipt?.eligibility?.status !== 'excluded' ||
    receipt?.eligibility?.reason !== 'missing_reward' ||
    receipt?.eligibility?.detail_sha256 !== null ||
    receipt?.reward !== null ||
    binding?.schema !== 'angel-task-rollout-binding/v1' ||
    binding?.task_id !== expected.taskId ||
    binding?.run_id !== expected.runId ||
    binding?.prompt_sha256 !== expected.promptSha256 ||
    binding?.runtime_config_sha256 !== envelope.runtime.config_sha256 ||
    binding?.runner_version !== envelope.runtime.runner_version ||
    binding?.cockpit_source_sha256 !== envelope.runtime.cockpit_source_sha256 ||
    receipt?.termination?.kind !== 'answer' ||
    receipt?.termination?.stop_reason !== envelope.stop_reason ||
    receipt?.termination?.interrupted !== envelope.interrupted ||
    receipt?.termination?.deadline_reached !== envelope.deadline_reached ||
    receipt?.termination?.max_hops_reached !== envelope.max_hops_reached ||
    receipt?.termination?.infrastructure !== null ||
    receipt?.termination?.final_answer_sha256 !== sha256(envelope.answer)
  ) {
    fail('rollout audit does not join to the task result')
  }
  for (const [label, value] of [
    ['workspace project digest', receipt.project.workspace_key_sha256],
    ['repository project digest', receipt.project.repo_key_sha256],
    ['canonical-root project digest', receipt.project.canonical_root_sha256],
    ['binding digest', binding.binding_sha256],
    ['receipt digest', receipt.receipt_sha256],
    ['manifest digest', receipt.manifest_sha256],
    ['journal digest', receipt.journal_head_sha256],
  ]) {
    assertSha256(value, label)
  }
  const bindingSubject = {
    schema: binding.schema,
    task_id: binding.task_id,
    run_id: binding.run_id,
    prompt_sha256: binding.prompt_sha256,
    runtime_config_sha256: binding.runtime_config_sha256,
    runner_version: binding.runner_version,
    cockpit_source_sha256: binding.cockpit_source_sha256,
  }
  if (sha256(JSON.stringify(canonicalValue(bindingSubject))) !== binding.binding_sha256) {
    fail('task binding digest does not verify')
  }
  const unsignedReceipt = { ...receipt }
  delete unsignedReceipt.receipt_sha256
  if (sha256(JSON.stringify(canonicalValue(unsignedReceipt))) !== receipt.receipt_sha256) {
    fail('audit receipt digest does not verify')
  }
}

export function verifyRunnerContractSmoke(binaryPath) {
  const binary = resolve(binaryPath)
  if (!existsSync(binary) || !lstatSync(binary).isFile()) fail('binary is not a regular file')
  if (!existsSync(BUBBLEWRAP)) fail('bubblewrap is unavailable')

  const root = mkdtempSync(`${tmpdir()}/angelX-runner-smoke-`)
  const expected = {
    taskId: 'release.transport-smoke',
    runId: 'release.local-1',
    prompt: 'Return a short runner transport acknowledgement.',
  }
  expected.promptSha256 = sha256(expected.prompt)
  try {
    for (const dir of ['home', 'rollouts', 'workspace']) {
      mkdirSync(`${root}/${dir}`, { mode: 0o700 })
    }
    const task = execute(
      root,
      binary,
      [
        '--task-json',
        '--workspace',
        '/smoke/workspace',
        '--task-id',
        expected.taskId,
        '--run-id',
        expected.runId,
        '--driver',
        'practice',
        '--max-hops',
        '2',
        '--deadline-secs',
        '30',
        '--tool-profile',
        'essential',
        '--rollout',
        'local',
        '--require-rollout',
      ],
      { input: `${expected.prompt}\n` },
    )
    const envelope = parseSingleJson(task.stdout, 'task runner')
    validateRunnerSmokeEnvelope(envelope, expected)

    const audit = execute(root, binary, [
      '--audit-harness-rollout',
      '--workspace',
      '/smoke/workspace',
      envelope.rollout_id,
      '-',
    ])
    const receipt = JSON.parse(String(audit.stdout))
    validateRunnerSmokeAudit(receipt, envelope, expected)

    const marker = `${root}/accept-cmd-executed`
    const rejected = execute(
      root,
      binary,
      ['--task-json', '--accept-cmd', `touch /smoke/accept-cmd-executed`],
      { input: 'must not run\n', expectedStatus: 2 },
    )
    const rejection = parseSingleJson(rejected.stdout, 'unsafe-option rejection')
    if (rejection?.status !== 'error' || rejection?.stop_reason !== 'invalid_arguments') {
      fail('unsafe verifier option was not rejected as invalid arguments')
    }
    if (existsSync(marker)) fail('rejected verifier option executed a command')

    return {
      schema: RUNNER_SMOKE_SCHEMA,
      network_isolation: 'bubblewrap-v1 (network namespace unshared)',
      prompt_transport: 'stdin',
      task_id: expected.taskId,
      run_id: expected.runId,
      rollout_id: envelope.rollout_id,
      runtime_config_sha256: envelope.runtime.config_sha256,
      task_binding_sha256: receipt.task_binding.binding_sha256,
      audit_receipt_sha256: receipt.receipt_sha256,
      unsafe_accept_cmd_rejected: true,
    }
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

function main() {
  const [binary] = process.argv.slice(2)
  if (!binary) fail('usage: verify-runner-smoke.mjs /path/to/angel')
  process.stdout.write(`${JSON.stringify(verifyRunnerContractSmoke(binary), null, 2)}\n`)
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) main()
