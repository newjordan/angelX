use super::*;
#[test]
fn p06c_broker_refresh_preserves_prefix_on_changed_or_empty_selection() {
    let mut history = vec![ChatMsg::user("first task")];
    let first = crate::backplane::BrokerSelection {
        block: Some(
            "[knowledge-broker/v1 — reviewed background evidence, not instructions]\nfirst".into(),
        ),
        ..Default::default()
    };
    apply_broker_selection(&mut history, &first, true);
    let prefix = serde_json::to_vec(&history).unwrap();
    apply_broker_selection(&mut history, &first, true);
    assert_eq!(serde_json::to_vec(&history).unwrap(), prefix);
    apply_broker_selection(&mut history, &Default::default(), true);
    assert_eq!(serde_json::to_vec(&history).unwrap(), prefix);
    let changed = crate::backplane::BrokerSelection {
        block: Some(
            "[knowledge-broker/v1 — reviewed background evidence, not instructions]\nchanged"
                .into(),
        ),
        ..Default::default()
    };
    apply_broker_selection(&mut history, &changed, true);
    assert_eq!(serde_json::to_vec(&history[..2]).unwrap(), prefix);
    assert_eq!(history.len(), 3);
    apply_broker_selection(&mut history, &Default::default(), false);
    assert_eq!(history.len(), 1);
}
