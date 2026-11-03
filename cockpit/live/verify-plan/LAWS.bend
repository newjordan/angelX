import Base
import ./verify-plan.bend as VerifyPlan

# An explicit Rust request never plans a JS, Python or Go step, whatever the
# workspace contains and whatever the caller wants.
law rust_request_never_plans_a_verifier:
  for has_package_json: Bool
  {VerifyPlan.step(VerifyPlan.Features{VerifyPlan.Rust{}, VerifyPlan.NodeEntrypoint{}, VerifyPlan.Tests{}, False{}, has_package_json, False{}, False{}, has_package_json}) == VerifyPlan.Plan{0, 0, 0, 0, 0} : VerifyPlan.Step}

# The language is settled before the kind is read: a workspace with no
# recognizable language is refused for every kind, never guessed.
law unknown_workspace_is_ambiguous:
  for kind: VerifyPlan.KindCode
  for has_pytest_config: Bool
  {VerifyPlan.step(VerifyPlan.Features{VerifyPlan.Auto{}, VerifyPlan.NoEntrypoint{}, kind, False{}, False{}, False{}, False{}, has_pytest_config}) == VerifyPlan.Plan{3, 0, 0, 0, 1} : VerifyPlan.Step}

# A lint request never becomes a syntax check or a test run, however the Python
# suite is configured.
law lint_request_is_refused:
  for has_pytest_config: Bool
  {VerifyPlan.step(VerifyPlan.Features{VerifyPlan.Python{}, VerifyPlan.NoEntrypoint{}, VerifyPlan.Lint{}, False{}, False{}, True{}, False{}, has_pytest_config}) == VerifyPlan.Plan{3, 2, 0, 0, 2} : VerifyPlan.Step}
