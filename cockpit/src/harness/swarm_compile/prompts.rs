pub(super) fn investigator(goal: &str, targeted_test: &str, accept: &str) -> String {
    format!(
        "Read and obey the active repository scope instructions first. Investigate only; do not edit files.\n\
         Goal: {goal}\n\
         Targeted proof command: {targeted_test}\n\
         Full acceptance command: {accept}\n\
         Return a compact evidence packet: relevant files/symbols, likely root cause or design seam, risks, and the smallest implementation boundary. Cite concrete paths."
    )
}

pub(super) fn test_author(
    goal: &str,
    findings: &str,
    test_scope: &[String],
    targeted_test: &str,
    red_marker: &str,
) -> String {
    format!(
        "Read and obey the active repository scope instructions first. Add the smallest standalone regression test for the goal.\n\
         Goal: {goal}\n\
         Investigator evidence: {findings}\n\
         You may edit ONLY these test paths/prefixes: {}\n\
         The orchestrator will run: {targeted_test}\n\
         The test must fail for the missing behavior, include the exact diagnostic marker `{red_marker}` in that intentional failure, and pass once the behavior is correctly implemented. Do not edit production code, weaken existing assertions, or make unrelated changes. Run the targeted command once if useful, then summarize the proof you added.",
        test_scope.join(", ")
    )
}

pub(super) fn implementer(
    goal: &str,
    findings: &str,
    protected_paths: &[String],
    targeted_test: &str,
    accept: &str,
) -> String {
    format!(
        "Read and obey the active repository scope instructions first. Implement the requested behavior on top of an immutable failing regression-test commit.\n\
         Goal: {goal}\n\
         Investigator evidence: {findings}\n\
         Protected test files (do not edit, replace, rename, or delete): {}\n\
         Targeted command: {targeted_test}\n\
         Full acceptance command: {accept}\n\
         Make the smallest production change that turns the protected test green. Preserve existing behavior and finish with a concise summary of changed production files and commands run.",
        protected_paths.join(", ")
    )
}

pub(super) fn reviewer(goal: &str, diff: &str, targeted_test: &str, accept: &str) -> String {
    format!(
        "Read and obey the active repository scope instructions first. Review this candidate from its exact branch; do not edit files.\n\
         Goal: {goal}\n\
         Targeted proof command: {targeted_test}\n\
         Full acceptance command: {accept}\n\
         Candidate diff from the original base:\n{diff}\n\n\
         Look for correctness gaps, scope violations, weakened tests, unsafe behavior, and needless complexity. Cite paths and executable objections. End with exactly one final line: `SWARM_REVIEW: PASS` if no blocking issue remains, otherwise `SWARM_REVIEW: BLOCK`."
    )
}
