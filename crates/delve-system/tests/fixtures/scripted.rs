//! A worker whose rule step calls a Bend module through `angel_delve::scripted`
//! and records the answer on the run. It is only built by the swap regression
//! suite, which proves the worker evaluates shipped modules over the real
//! protocol.
fn main() {
    angel_delve_system::serve_with(|run, inputs| {
        run.step(inputs);
        let probe = serde_json::json!({
            "seat": "probe", "native": "low", "allowed": ["low", "high"], "hop": 0,
            "friction": false, "tool_errors": 0, "failed_checks": 0,
            "task": "fix the failing test", "task_len": 20
        });
        let answer: Result<serde_json::Value, String> =
            angel_delve::scripted::step("effort", &probe);
        run.cues.push(format!("scripted:{answer:?}"));
    })
    .unwrap();
}
