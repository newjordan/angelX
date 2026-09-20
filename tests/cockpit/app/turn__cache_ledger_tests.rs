use super::*;

fn reported(hop: usize, input: u64, cache_read: u64) -> HopCacheSample {
    HopCacheSample {
        hop,
        input,
        cache_read,
        cache_write: 0,
        reported: true,
        breakers: Vec::new(),
    }
}

#[test]
fn first_reported_hop_never_notices_and_totals_accumulate() {
    let mut ledger = ClubCacheLedger::default();
    let notice = ledger.fold_hop("glm", &reported(1, 10_000, 9_000), 20);
    assert!(notice.is_none(), "no baseline yet — nothing to drop from");
    assert_eq!(ledger.hops, 1);
    assert_eq!(ledger.reported_hops, 1);
    assert_eq!(ledger.input, 10_000);
    assert_eq!(ledger.cache_read, 9_000);
}

#[test]
fn drop_names_the_tagged_breakers_and_counts_attribution() {
    let mut ledger = ClubCacheLedger::default();
    assert!(
        ledger
            .fold_hop("glm", &reported(1, 10_000, 9_000), 20)
            .is_none()
    );
    let mut broken = reported(2, 10_000, 3_000);
    broken.breakers = vec!["defs delta", "steer injection"];
    let notice = ledger
        .fold_hop("glm", &broken, 20)
        .expect("a 60-point fall past a 20-point threshold notices");
    assert_eq!(
        notice,
        "cache ledger: glm hop 2 prefix read 9000 → 3000; hit 90% → 30% after defs delta + steer injection"
    );
    assert_eq!(ledger.drops, 1);
    assert_eq!(ledger.breaker_drops.get("defs delta"), Some(&1));
    assert_eq!(ledger.breaker_drops.get("steer injection"), Some(&1));
    assert_eq!(ledger.unattributed_drops, 0);
}

#[test]
fn untagged_drop_says_so_instead_of_staying_silent() {
    let mut ledger = ClubCacheLedger::default();
    assert!(
        ledger
            .fold_hop("glm", &reported(1, 10_000, 9_000), 20)
            .is_none()
    );
    let notice = ledger
        .fold_hop("glm", &reported(2, 10_000, 0), 20)
        .expect("a full miss notices");
    assert!(
        notice.contains("no tagged prefix breaker"),
        "unattributed drops must name their own uncertainty: {notice}"
    );
    assert_eq!(ledger.unattributed_drops, 1);
}

#[test]
fn growing_uncached_tail_dilutes_ratio_without_claiming_prefix_loss() {
    let mut ledger = ClubCacheLedger::default();
    assert!(
        ledger
            .fold_hop("glm", &reported(1, 7_829, 7_808), 20)
            .is_none()
    );
    let notice = ledger
        .fold_hop("glm", &reported(2, 15_626, 7_808), 20)
        .expect("a significant dilution stays visible but is not a drop");
    assert_eq!(
        notice,
        "cache ledger: glm hop 2 hit 100% → 50% from uncached tail growth; prefix read held 7808 → 7808"
    );
    assert_eq!(ledger.drops, 0);
    assert_eq!(ledger.dilutions, 1);
    assert_eq!(ledger.unattributed_drops, 0);
    assert!(ledger.usage_row("glm").contains("1 tail dilution(s)"));
}

#[test]
fn falls_below_threshold_and_rises_never_notice() {
    let mut ledger = ClubCacheLedger::default();
    assert!(
        ledger
            .fold_hop("glm", &reported(1, 10_000, 9_000), 20)
            .is_none()
    );
    // 90% → 80% is under the 20-point threshold.
    assert!(
        ledger
            .fold_hop("glm", &reported(2, 10_000, 8_000), 20)
            .is_none()
    );
    // Recovery is never a drop.
    assert!(
        ledger
            .fold_hop("glm", &reported(3, 10_000, 9_500), 20)
            .is_none()
    );
    assert_eq!(ledger.drops, 0);
}

#[test]
fn zero_threshold_still_requires_an_actual_fall() {
    let mut ledger = ClubCacheLedger::default();
    assert!(
        ledger
            .fold_hop("glm", &reported(1, 10_000, 9_000), 0)
            .is_none()
    );
    // Identical rate: no fall, no notice even at threshold 0.
    assert!(
        ledger
            .fold_hop("glm", &reported(2, 10_000, 9_000), 0)
            .is_none()
    );
    // A 1-point fall does notice at threshold 0 (treated as 1).
    assert!(
        ledger
            .fold_hop("glm", &reported(3, 10_000, 8_900), 0)
            .is_some()
    );
}

#[test]
fn tiny_denominators_are_too_noisy_to_indict_a_breaker() {
    let mut ledger = ClubCacheLedger::default();
    assert!(ledger.fold_hop("glm", &reported(1, 400, 380), 20).is_none());
    // 95% → 0% but over 400 tokens: numerically meaningless, exempt.
    assert!(ledger.fold_hop("glm", &reported(2, 400, 0), 20).is_none());
    assert_eq!(ledger.drops, 0);
}

#[test]
fn unreported_hops_accumulate_totals_but_never_move_the_baseline() {
    let mut ledger = ClubCacheLedger::default();
    assert!(
        ledger
            .fold_hop("glm", &reported(1, 10_000, 9_000), 20)
            .is_none()
    );
    let mut blind = reported(2, 5_000, 0);
    blind.reported = false;
    assert!(ledger.fold_hop("glm", &blind, 20).is_none());
    assert_eq!(ledger.hops, 2);
    assert_eq!(ledger.reported_hops, 1);
    assert_eq!(ledger.input, 15_000, "blind tokens still count in totals");
    // The next reported hop compares against hop 1's 90%, not the blind hop.
    assert!(
        ledger
            .fold_hop("glm", &reported(3, 10_000, 3_000), 20)
            .is_some()
    );
}

#[test]
fn usage_row_covers_blind_and_metered_routes() {
    let mut ledger = ClubCacheLedger::default();
    let mut blind = reported(1, 5_000, 0);
    blind.reported = false;
    assert!(ledger.fold_hop("nex2", &blind, 20).is_none());
    assert_eq!(ledger.usage_row("nex2"), "nex2 · 1 hop(s) · cache n/a");

    let mut metered = ClubCacheLedger::default();
    assert!(
        metered
            .fold_hop("glm", &reported(1, 10_000, 9_000), 20)
            .is_none()
    );
    let mut broken = reported(2, 10_000, 3_000);
    broken.breakers = vec!["compaction splice"];
    assert!(metered.fold_hop("glm", &broken, 20).is_some());
    assert!(
        metered
            .fold_hop("glm", &reported(3, 10_000, 0), 20)
            .is_some()
    );
    let row = metered.usage_row("glm");
    // Legacy counters have no qualified cache convention; raw totals do
    // not justify a percentage even when every hop reports numbers.
    assert_eq!(
        row,
        "glm · 3 hop(s) (3 reported) · hit n/a · in 30k · read 12k · 2 drop(s) \
             → compaction splice ×1, untagged ×1"
    );
}

#[test]
fn fmt_tokens_scales_readably() {
    assert_eq!(fmt_tokens(0), "0");
    assert_eq!(fmt_tokens(9_999), "9999");
    assert_eq!(fmt_tokens(10_000), "10k");
    assert_eq!(fmt_tokens(999_999), "999k");
    assert_eq!(fmt_tokens(1_250_000), "1.2M");
}

#[test]
fn defs_fingerprint_tracks_the_offered_schema_set() {
    let def = |name: &str, description: &str| ToolDef {
        name: name.to_string(),
        description: description.to_string(),
        params: serde_json::json!({}),
    };
    let base = vec![def("read_file", "read"), def("shell", "run")];
    assert_eq!(defs_fingerprint(&base), defs_fingerprint(&base.clone()));
    let renamed = vec![def("read_file", "read"), def("shell_v2", "run")];
    assert_ne!(defs_fingerprint(&base), defs_fingerprint(&renamed));
    let grown = vec![
        def("read_file", "read"),
        def("shell", "run"),
        def("lsp", "query"),
    ];
    assert_ne!(defs_fingerprint(&base), defs_fingerprint(&grown));
}
