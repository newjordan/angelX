import Base
import ./verify-plan.bend as VerifyPlan

law rust_request_never_plans_a_verifier:
  for has_package_json: Bool
  {VerifyPlan.step(VerifyPlan.Features{VerifyPlan.Rust{}, VerifyPlan.NodeEntrypoint{}, VerifyPlan.Tests{}, False{}, has_package_json, False{}, False{}, has_package_json}) == VerifyPlan.Plan{0, 0, 0, 0, 0} : VerifyPlan.Step}

law unknown_workspace_is_ambiguous:
  for kind: VerifyPlan.KindCode
  for has_pytest_config: Bool
  {VerifyPlan.step(VerifyPlan.Features{VerifyPlan.Auto{}, VerifyPlan.NoEntrypoint{}, kind, False{}, False{}, False{}, False{}, has_pytest_config}) == VerifyPlan.Plan{3, 0, 0, 0, 1} : VerifyPlan.Step}

law lint_request_is_refused:
  for has_pytest_config: Bool
  {VerifyPlan.step(VerifyPlan.Features{VerifyPlan.Python{}, VerifyPlan.NoEntrypoint{}, VerifyPlan.Lint{}, False{}, False{}, True{}, False{}, has_pytest_config}) == VerifyPlan.Plan{3, 2, 0, 0, 2} : VerifyPlan.Step}
