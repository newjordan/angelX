// Thin adapter: the host sends the request and workspace facts as plain JSON,
// the checked Bend module decides, and the step codes return unchanged. No
// planning lives here.
const { step } = require('./verify-plan.compiled.js')

const REQUESTED = { auto: 'Auto', rust: 'Rust', go: 'Go', node: 'Node', python: 'Python' }
const ENTRYPOINT = { none: 'NoEntrypoint', node: 'NodeEntrypoint', unittest: 'Unittest', pytest: 'Pytest', other: 'Other' }
const KIND = { check: 'Check', tests: 'Tests', lint: 'Lint' }

const flag = value => value === true
const one = (table, value) => {
  const tag = table[value]
  if (!tag) throw new Error(`verify-plan: unknown feature value ${JSON.stringify(value)}`)
  return { $: tag }
}

module.exports = {
  apiVersion: 1,
  render: input => {
    const decision = step({
      $: 'Features',
      requested: one(REQUESTED, input.requested),
      entrypoint: one(ENTRYPOINT, input.entrypoint),
      kind: one(KIND, input.kind),
      has_cargo_toml: flag(input.has_cargo_toml),
      has_package_json: flag(input.has_package_json),
      has_pyproject_toml: flag(input.has_pyproject_toml),
      has_python_setup: flag(input.has_python_setup),
      has_pytest_config: flag(input.has_pytest_config),
    })
    return {
      route: decision.route,
      runtime: decision.runtime,
      mode: decision.mode,
      driver: decision.driver,
      reject: decision.reject,
    }
  },
}
