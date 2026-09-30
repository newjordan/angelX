//! The proof graph's role prompts. Each role is a delegate seat that reads the
//! ledger through its own `read_file`: its words are its route on `⠠` (the
//! investigator `⠠⠛`, the test author `⠠⠓`, the implementer `⠠⠊`, the reviewer
//! `⠠⠚`), and the goal, evidence, scopes, commands and diff ride after their
//! labels' page addresses as data.

use crate::agent::harness::book::d6_long_run::{IMPLEMENTER, INVESTIGATOR, REVIEWER, TEST_AUTHOR};

pub(super) fn investigator(goal: &str, targeted_test: &str, accept: &str) -> String {
    let role = INVESTIGATOR.cells();
    format!("{role}\n{role}⠉ {goal}\n{role}⠙ {targeted_test}\n{role}⠑ {accept}")
}

pub(super) fn test_author(
    goal: &str,
    findings: &str,
    test_scope: &[String],
    targeted_test: &str,
    red_marker: &str,
) -> String {
    let role = TEST_AUTHOR.cells();
    format!(
        "{role} red_marker={red_marker}\n{role}⠉ {goal}\n{role}⠙ {findings}\n{role}⠑ {}\n\
         {role}⠋ {targeted_test}",
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
    let role = IMPLEMENTER.cells();
    format!(
        "{role}\n{role}⠉ {goal}\n{role}⠙ {findings}\n{role}⠑ {}\n{role}⠋ {targeted_test}\n\
         {role}⠛ {accept}",
        protected_paths.join(", ")
    )
}

pub(super) fn reviewer(goal: &str, diff: &str, targeted_test: &str, accept: &str) -> String {
    let role = REVIEWER.cells();
    format!("{role}\n{role}⠉ {goal}\n{role}⠙ {targeted_test}\n{role}⠑ {accept}\n{role}⠋\n{diff}")
}
