//! Review saved Bard rule states with the production half-block/braille painter.
#[test]
#[ignore = "writes Bard native frames and actual cockpit terminal cells"]
fn write_bard_terminal_shots() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_BARD_REVIEW").unwrap());
    for name in ["stream", "song"] {
        let run: angel_delve::rules::Run = serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap()).unwrap();
        assert_eq!(run.players.len(), 4);
        super::tests::capture(&dir, &format!("{name}-production"), &run, true);
    }
}
