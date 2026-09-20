#[test]
fn delegate_token_accounting_does_not_charge_reasoning_twice() {
    let before = crate::agent::club::TokenUsage {
        total_input: 100,
        total_output: 20,
        total_reasoning: 12,
        ..Default::default()
    };
    let after = crate::agent::club::TokenUsage {
        total_input: 400,
        total_output: 80,
        total_reasoning: 50,
        ..Default::default()
    };
    assert_eq!(delegate_token_delta(before, after), 360);
    assert_eq!(delegate_token_delta(after, before), 0);
}
