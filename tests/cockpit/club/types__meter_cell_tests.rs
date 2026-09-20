use super::*;

#[test]
fn usage_cell_record_turn_accumulates_without_a_mutex() {
    let cell = UsageCell::default();
    assert_eq!(cell.load(), TokenUsage::default());
    cell.record_turn(10, 4, 1);
    cell.record_turn(3, 2, 0);
    assert_eq!(
        cell.load(),
        TokenUsage {
            turns: 2,
            last_input: 3,
            last_output: 2,
            last_reasoning: 0,
            total_input: 13,
            total_output: 6,
            total_reasoning: 1,
        }
    );
}

#[test]
fn cache_usage_cell_adds_read_write_and_control_counts() {
    let cell = CacheUsageCell::default();
    cell.add_control_request();
    cell.add_read(40);
    cell.add_write(8);
    assert_eq!(
        cell.load(),
        CacheUsage {
            control_requests: 1,
            read_input_tokens: 40,
            write_input_tokens: 8,
            read_accounting_responses: 1,
            write_accounting_responses: 1,
        }
    );
}
