#[test]
fn t04b_prefixed_recipe_preserves_directory_and_environment() {
    let call = crate::agent::club::ToolCall {
        id: "prefix".into(),
        name: "run_tests".into(),
        args: serde_json::json!({"runtime":"python", "entrypoint":"unittest", "args":"test_a.py", "dir":"suite", "env":{"TEST_MODE":"fixture"}}),
    };
    let (command, env) = super::command_of(&call).unwrap();
    assert!(command.contains("\"dir\":\"suite\""));
    assert_eq!(env, vec!["TEST_MODE=fixture"]);
}
