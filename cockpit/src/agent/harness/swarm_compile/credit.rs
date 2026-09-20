use super::schema::{CreditRow, ReviewVerdict, SwarmRun};

pub(super) fn review_verdict(text: &str) -> ReviewVerdict {
    for line in text.lines().rev() {
        let normalized = line
            .trim()
            .trim_matches(|c: char| c == '`' || c == '*' || c == '-');
        if normalized.eq_ignore_ascii_case("SWARM_REVIEW: PASS") {
            return ReviewVerdict::Pass;
        }
        if normalized.eq_ignore_ascii_case("SWARM_REVIEW: BLOCK") {
            return ReviewVerdict::Block;
        }
    }
    // Review is a safety gate. A malformed verdict cannot silently approve.
    ReviewVerdict::Block
}

pub(super) fn derive_credits(run: &SwarmRun) -> Vec<CreditRow> {
    let mut rows = Vec::new();
    if let Some(test_author) = run.contribution("test_author") {
        rows.push(CreditRow {
            role: test_author.role.clone(),
            club: test_author.club.clone(),
            reward: f64::from(run.verification.differential),
            basis: "test failed with the run marker before implementation and passed afterward"
                .to_string(),
            tokens: test_author.tokens,
            elapsed_ms: test_author.elapsed_ms,
        });
    }
    if let Some(implementer) = run.contribution("implementer") {
        rows.push(CreditRow {
            role: implementer.role.clone(),
            club: implementer.club.clone(),
            reward: f64::from(run.verification.technical_pass),
            basis: "implementation moved the protected regression from red to green and passed the full gate"
                .to_string(),
            tokens: implementer.tokens,
            elapsed_ms: implementer.elapsed_ms,
        });
    }
    if let Some(reviewer) = run.contribution("reviewer") {
        let expected = if run.verification.technical_pass {
            ReviewVerdict::Pass
        } else {
            ReviewVerdict::Block
        };
        rows.push(CreditRow {
            role: reviewer.role.clone(),
            club: reviewer.club.clone(),
            reward: f64::from(reviewer.review_verdict == Some(expected)),
            basis: "review verdict calibrated against the post-action deterministic gate"
                .to_string(),
            tokens: reviewer.tokens,
            elapsed_ms: reviewer.elapsed_ms,
        });
    }
    rows
}
