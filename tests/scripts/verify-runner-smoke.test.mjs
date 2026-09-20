import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import test from 'node:test'

import { validateRunnerSmokeAudit, validateRunnerSmokeEnvelope } from '../../scripts/verify-runner-smoke.mjs'

function canonicalValue(value) {
  if (Array.isArray(value)) return value.map(canonicalValue)
  if (!value || typeof value !== 'object') return value
  return Object.fromEntries(
    Object.keys(value)
      .sort()
      .map((key) => [key, canonicalValue(value[key])]),
  )
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex')
}

function digest(value) {
  return sha256(JSON.stringify(canonicalValue(value)))
}

function fixture() {
  const expected = {
    taskId: 'release.transport-smoke',
    runId: 'release.local-1',
    prompt: 'Return a short runner transport acknowledgement.',
  }
  expected.promptSha256 = sha256(expected.prompt)
  const runtimeMaterial = {
    schema: 'angel-task-runtime/v1',
    runner_version: '1.1.33',
    cockpit_source_sha256: 'unbound',
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
  const envelope = {
    version: 1,
    kind: 'angel.task_result',
    task_id: expected.taskId,
    run_id: expected.runId,
    workspace: '/smoke/workspace',
    status: 'completed',
    stop_reason: 'answer',
    answer: 'acknowledged',
    interrupted: false,
    deadline_reached: false,
    max_hops_reached: false,
    rollout_id: 'rol-a-b-c',
    runtime: {
      ...Object.fromEntries(Object.entries(runtimeMaterial).filter(([, value]) => value !== null)),
      config_sha256: digest(runtimeMaterial),
    },
  }
  const bindingMaterial = {
    schema: 'angel-task-rollout-binding/v1',
    task_id: expected.taskId,
    run_id: expected.runId,
    prompt_sha256: expected.promptSha256,
    runtime_config_sha256: envelope.runtime.config_sha256,
    runner_version: envelope.runtime.runner_version,
    cockpit_source_sha256: envelope.runtime.cockpit_source_sha256,
  }
  const receipt = {
    schema: 'angel-harness-rollout-audit/v1',
    rollout_id: envelope.rollout_id,
    project: {
      schema: 'angel-project-digest/v1',
      workspace_key_sha256: 'a'.repeat(64),
      repo_key_sha256: 'b'.repeat(64),
      canonical_root_sha256: 'c'.repeat(64),
    },
    task_binding: { ...bindingMaterial, binding_sha256: digest(bindingMaterial) },
    capture: {
      mode: 'local',
      semantic_bodies: true,
      media_bodies: false,
      private_reasoning_captured: false,
      provider_headers_captured: false,
      recorder_revision: envelope.runtime.runner_version,
    },
    status: 'finalized',
    eligibility: { status: 'excluded', reason: 'missing_reward', detail_sha256: null },
    requested_route: {
      driver: 'practice',
      model_revision: 'practice',
      reasoning_effort: null,
    },
    termination: {
      kind: 'answer',
      stop_reason: 'answer',
      interrupted: false,
      deadline_reached: false,
      max_hops_reached: false,
      infrastructure: null,
      final_answer_sha256: sha256(envelope.answer),
    },
    reward: null,
    attempt_count: 1,
    event_count: 2,
    started_ms: 1,
    sealed_ms: 2,
    journal_head_sha256: 'd'.repeat(64),
    manifest_sha256: 'e'.repeat(64),
    compatibility_present: false,
  }
  receipt.receipt_sha256 = digest(receipt)
  return { expected, envelope, receipt }
}

test('runner smoke validates the complete runtime digest and audit receipt', () => {
  const { expected, envelope, receipt } = fixture()
  assert.doesNotThrow(() => validateRunnerSmokeEnvelope(envelope, expected))
  assert.doesNotThrow(() => validateRunnerSmokeAudit(receipt, envelope, expected))
})

test('runner smoke rejects a forged runtime config digest', () => {
  const { expected, envelope } = fixture()
  envelope.runtime.config_sha256 = 'f'.repeat(64)
  assert.throws(() => validateRunnerSmokeEnvelope(envelope, expected), /digest does not verify/u)
})

test('runner smoke rejects a tampered audit receipt', () => {
  const { expected, envelope, receipt } = fixture()
  receipt.attempt_count += 1
  assert.throws(
    () => validateRunnerSmokeAudit(receipt, envelope, expected),
    /digest does not verify/u,
  )
})

test('runner smoke rejects audit receipts that claim private reasoning capture', () => {
  const { expected, envelope, receipt } = fixture()
  receipt.capture.private_reasoning_captured = true
  const unsigned = { ...receipt }
  delete unsigned.receipt_sha256
  receipt.receipt_sha256 = digest(unsigned)
  assert.throws(() => validateRunnerSmokeAudit(receipt, envelope, expected), /does not join/u)
})
