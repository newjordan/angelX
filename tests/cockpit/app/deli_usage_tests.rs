use super::*;
use crate::club::{AccountingCell, AccountingView, UsageObservation};

#[derive(Default)]
struct MeteredClub {
    accounting: AccountingCell,
    fail: bool,
}
impl Club for MeteredClub {
    fn label(&self) -> &str {
        "owned-metered"
    }
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        let mut attempt = self.accounting.attempt();
        if self.fail {
            return Err("owned provider failure".into());
        }
        attempt.observe(Some(UsageObservation {
            raw: [Some(11), Some(2), None, None, None],
            paths: [
                Some("input_tokens"),
                Some("output_tokens"),
                None,
                None,
                None,
            ],
            ..Default::default()
        }));
        Ok("owned answer".into())
    }
    fn usage_accounting(&self) -> AccountingView {
        self.accounting.view()
    }
}

#[test]
fn deli_usage_accounting_forwards_attempts_without_duplicate_aliases() {
    let inner = Arc::new(MeteredClub::default());
    inner.respond("earlier unrelated call").unwrap();
    let deli = DeliClub::with_knobs(
        "deli",
        inner.clone(),
        Knobs {
            rounds: 1,
            ..Knobs::default()
        },
    );
    let before = deli.usage_accounting();
    // Actual Deli iteration plus synthesis: two distinct inner attempts.
    deli.respond("owned bounded task").unwrap();
    let mut after = deli.usage_accounting();
    after.extend(inner.usage_accounting());
    let report = after.delta(&before);
    assert_eq!(
        (report.attempts, report.input, report.output),
        (2, Some(22), Some(4))
    );
    assert!(report.core_complete);
    assert!(!report.untracked_sources);
    assert_eq!(report.attribution, "shared-cumulative-window");
}

#[test]
fn deli_usage_accounting_preserves_failed_unknown_attempt() {
    let inner = Arc::new(MeteredClub {
        fail: true,
        ..Default::default()
    });
    let deli = DeliClub::with_knobs("deli", inner, Knobs::default());
    let before = deli.usage_accounting();
    assert!(deli.respond("").is_err());
    let report = deli.usage_accounting().delta(&before);
    assert_eq!(report.attempts, 1);
    assert_eq!((report.input, report.output), (None, None));
    assert!(!report.core_complete);
    assert!(!report.untracked_sources);
}

#[test]
fn deli_usage_accounting_preserves_untracked_inner() {
    struct Untracked;
    impl Club for Untracked {
        fn label(&self) -> &str {
            "owned-untracked"
        }
        fn respond(&self, _prompt: &str) -> Result<String, String> {
            Ok("owned answer".into())
        }
    }
    let deli = DeliClub::with_knobs("deli", Arc::new(Untracked), Knobs::default());
    let before = deli.usage_accounting();
    deli.respond("").unwrap();
    let report = deli.usage_accounting().delta(&before);
    assert!(report.untracked_sources);
    assert!(!report.core_complete);
    assert_eq!((report.input, report.output), (None, None));
}
