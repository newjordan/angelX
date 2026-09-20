use super::*;

#[test]
fn spawn_prompt_batches_only_tool_bearing_seats() {
    for grant in [Grant::None, Grant::ReadOnly, Grant::Code, Grant::Research] {
        let prompt = seat_system("reviewer", "", Formation::Panel, 2, grant);
        assert_eq!(prompt.contains(TOOL_BATCHING_HINT), grant != Grant::None);
        assert!(!prompt.contains("code_mode"));
    }
}
