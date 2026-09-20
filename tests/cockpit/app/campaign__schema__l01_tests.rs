use super::*;
#[test]
fn campaign_defaults_do_not_supply_run_caps() {
    let policy = CampaignPolicy::default();
    assert_eq!(policy.max_rounds, 0);
    assert_eq!(policy.token_budget, 0);
    assert_eq!(policy.deadline_secs, 0);
}
