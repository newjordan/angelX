use super::*;

#[test]
fn spawn_prompt_batches_only_tool_bearing_seats() {
    for grant in [Grant::None, Grant::ReadOnly, Grant::Code, Grant::Research] {
        let prompt = seat_system("reviewer", "", Formation::Panel, 2, grant);
        // A tool-bearing seat carries the batching page `⠺⠉⠁`; a bare seat keeps prose.
        assert_eq!(prompt.contains("⠺⠉⠁"), grant != Grant::None);
        assert!(!prompt.contains("code_mode"));
    }
}
