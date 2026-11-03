import Base
import ./verify-policy.bend as Verify

# A call that never mutated the workspace cannot owe verification, whatever its
# target counts say.
law no_mutation_never_requires_verification:
  for targets: U32
  for code_targets: U32
  {Verify.decide_parts(Verify.Other{}, False{}, Verify.NoVerb{}, False{}, False{}, targets, code_targets, False{}, False{}, Nil{}, Nil{}) == Verify.Decision{False{}, False{}, False{}, False{}} : Verify.Decision}

# A verifier-looking segment behind an asynchronous `&` job reports launch, not
# completion: it is not verification evidence, and not progress credit either.
# It still counts as "tests ran somewhere".
law async_shell_is_not_verification_evidence:
  {Verify.decide_parts(Verify.Shell{}, False{}, Verify.NoVerb{}, False{}, False{}, 0, 0, False{}, True{}, Con{Verify.RunsVerifier{}, Nil{}}, Con{Verify.RunsVerifier{}, Nil{}}) == Verify.Decision{False{}, False{}, False{}, True{}} : Verify.Decision}

# One clean status-bearing segment that runs a verifier is verification evidence
# and progress credit at once.
law clean_shell_segment_verifies:
  {Verify.decide_parts(Verify.Shell{}, False{}, Verify.NoVerb{}, False{}, False{}, 0, 0, False{}, False{}, Con{Verify.RunsVerifier{}, Nil{}}, Nil{}) == Verify.Decision{False{}, True{}, True{}, False{}} : Verify.Decision}

# `cargo build` earns progress credit without satisfying the separate
# verify-before-done contract.
law cargo_build_is_progress_not_verification:
  {Verify.decide_parts(Verify.Cargo{}, False{}, Verify.Build{}, False{}, False{}, 0, 0, False{}, False{}, Nil{}, Nil{}) == Verify.Decision{False{}, False{}, True{}, False{}} : Verify.Decision}
