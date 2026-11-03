//! A file edit inside the live module root hands the agent the loader's verdict.

use super::*;

const ENVELOPE: &str = "[live-module/v1]";
const WORKING: &str = "module.exports={apiVersion:1,render: input => 'hello '+input.name}";

struct Rig {
    workspace: PathBuf,
    registry: ToolRegistry,
    client: angel_live::Client,
}

impl Rig {
    fn new(tag: &str) -> Self {
        let workspace = scratch(tag).canonicalize().unwrap();
        let module = workspace.join("live/report");
        std::fs::create_dir_all(&module).unwrap();
        std::fs::write(
            module.join("module.json"),
            r#"{"api_version":1,"id":"report","kind":"text","entry":"index.js","probe":{"name":"probe"}}"#,
        )
        .unwrap();
        std::fs::write(module.join("index.js"), WORKING).unwrap();
        let mut registry = ToolRegistry::new();
        registry.set_workspace(workspace.clone());
        register_file_tools(&mut registry, workspace.clone());
        let client =
            angel_live::Client::start(workspace.join("live"), crate::agent::hotload::evaluate);
        let until = Instant::now() + std::time::Duration::from_secs(5);
        while client
            .snapshot()
            .is_none_or(|snapshot| snapshot.revision == 0)
        {
            assert!(Instant::now() < until, "loader never published");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        registry.set_live_outcomes(Some((client.root.clone(), client.board())));
        Self {
            workspace,
            registry,
            client,
        }
    }

    fn write(&self, path: &str, content: &str) -> Result<String, String> {
        self.registry.dispatch(
            "write_file",
            &serde_json::json!({"path": path, "content": content}),
        )
    }

    fn verdict(text: &str) -> Vec<&str> {
        text.split_once(ENVELOPE)
            .map(|(_, verdict)| verdict.lines().filter(|line| !line.is_empty()).collect())
            .unwrap_or_default()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.workspace);
    }
}

#[test]
fn an_accepted_live_edit_returns_the_next_generation_with_the_tool_result() {
    let rig = Rig::new("live_accept");
    let result = rig
        .write(
            "live/report/index.js",
            "module.exports={apiVersion:1,render:()=>'one'}",
        )
        .unwrap();
    assert!(result.starts_with("wrote "), "{result}");
    let lines = Rig::verdict(&result);
    assert_eq!(lines.len(), 2, "{result}");
    assert_eq!(
        lines[0],
        format!("live modules · {}", rig.client.root.display())
    );
    let digest = &rig.client.snapshot().unwrap().statuses[0].digest;
    assert!(
        lines[1].starts_with(&format!("report · generation 2 · {} · ", &digest[..12])),
        "{result}"
    );
    assert!(lines[1].ends_with(" ms"), "{result}");
    // Every file tool that writes reports the same way.
    let edited = rig
        .registry
        .dispatch(
            "str_replace",
            &serde_json::json!({"path":"live/report/index.js","old":"'one'","new":"'two'"}),
        )
        .unwrap();
    assert!(
        Rig::verdict(&edited)[1].starts_with("report · generation 3 · "),
        "{edited}"
    );
    let multi = rig
        .registry
        .dispatch(
            "multi_edit",
            &serde_json::json!({"path":"live/report/index.js","edits":[{"old":"'two'","new":"'three'"}]}),
        )
        .unwrap();
    assert!(
        Rig::verdict(&multi)[1].starts_with("report · generation 4 · "),
        "{multi}"
    );
    let patched = rig
        .registry
        .dispatch(
            "apply_patch",
            &serde_json::json!({"diff":"*** Begin Patch\n*** Update File: live/report/index.js\n@@\n-module.exports={apiVersion:1,render:()=>'three'}\n+module.exports={apiVersion:1,render:()=>'four'}\n*** End Patch\n"}),
        )
        .unwrap();
    assert!(
        Rig::verdict(&patched)[1].starts_with("report · generation 5 · "),
        "{patched}"
    );
}

#[test]
fn a_broken_live_edit_returns_the_kept_generation_and_the_error() {
    let rig = Rig::new("live_reject");
    let before = rig.client.snapshot().unwrap().statuses[0].clone();
    let result = rig
        .write("live/report/index.js", "module.exports={broken:")
        .unwrap();
    let lines = Rig::verdict(&result);
    assert!(
        lines[1].starts_with(&format!(
            "report · generation {} · {} · ",
            before.generation,
            &before.digest[..12]
        )),
        "{result}"
    );
    assert!(lines[1].contains(" ms · kept previous: "), "{result}");
    // The previous generation still serves.
    assert_eq!(
        rig.client
            .request(Some("report"), serde_json::json!({"name":"still"}))
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .unwrap(),
        "hello still"
    );
}

#[test]
fn an_edit_outside_the_live_root_returns_no_verdict() {
    let rig = Rig::new("live_outside");
    std::fs::create_dir_all(rig.workspace.join("src")).unwrap();
    for path in [
        "notes.txt",
        "src/index.js",
        "live-notes/report/index.js",
        // A file directly in the root belongs to no module.
        "live/notes.txt",
    ] {
        let started = Instant::now();
        let result = rig.write(path, "module.exports={broken:").unwrap();
        assert!(!result.contains(ENVELOPE), "{path}: {result}");
        assert!(
            started.elapsed() < std::time::Duration::from_millis(500),
            "{path}"
        );
    }
    // A failed tool call opens nothing either.
    let missing = rig.registry.dispatch(
        "str_replace",
        &serde_json::json!({"path":"live/report/index.js","old":"absent text","new":"x"}),
    );
    assert!(missing.is_err());
    assert!(rig.registry.take_live_outcomes().is_none());
    // Without a loader bound, a live-looking path is an ordinary write.
    rig.registry.set_live_outcomes(None);
    assert!(
        !rig.write("live/report/index.js", WORKING)
            .unwrap()
            .contains(ENVELOPE)
    );
}

/// Write through `write_file` and cancel the call while it waits for the loader:
/// the same outcome as a verdict slower than the activation window.
fn write_then_stop_waiting(rig: &Rig, content: &str) -> String {
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let path = rig.workspace.join("live/report/index.js");
            while std::fs::read_to_string(&path).is_ok_and(|held| held != content) {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            stop.store(true, Ordering::Relaxed);
        });
        rig.registry
            .dispatch_with_cancel(
                "write_file",
                &serde_json::json!({"path":"live/report/index.js","content":content}),
                Some(&stop),
            )
            .unwrap()
    })
}

#[test]
fn a_verdict_after_the_window_rides_the_next_tool_result_once() {
    let rig = Rig::new("live_late");
    let result = write_then_stop_waiting(&rig, "module.exports={apiVersion:1,render:()=>'late'}");
    assert!(!result.contains(ENVELOPE), "{result}");
    let until = Instant::now() + std::time::Duration::from_secs(5);
    let carried = loop {
        let read = rig
            .registry
            .dispatch(
                "read_file",
                &serde_json::json!({"path":"live/report/index.js"}),
            )
            .unwrap();
        if read.contains(ENVELOPE) {
            break read;
        }
        assert!(Instant::now() < until, "verdict never arrived");
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(
        Rig::verdict(&carried)[1].starts_with("report · generation 2 · "),
        "{carried}"
    );
    let again = rig
        .registry
        .dispatch(
            "read_file",
            &serde_json::json!({"path":"live/report/index.js"}),
        )
        .unwrap();
    assert!(!again.contains(ENVELOPE), "{again}");
    assert!(rig.registry.take_live_outcomes().is_none());
    // A verdict no tool result carried reaches the next turn's context instead.
    let result = write_then_stop_waiting(&rig, "module.exports={apiVersion:1,render:()=>'later'}");
    assert!(!result.contains(ENVELOPE), "{result}");
    let until = Instant::now() + std::time::Duration::from_secs(5);
    let context = loop {
        if let Some(context) = rig.registry.take_live_outcomes() {
            break context;
        }
        assert!(Instant::now() < until, "verdict never arrived");
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(context.starts_with(ENVELOPE), "{context}");
    assert!(
        Rig::verdict(&context)[1].starts_with("report · generation 3 · "),
        "{context}"
    );
}

#[test]
fn the_registry_exposes_the_live_root_it_reports_against() {
    let rig = Rig::new("live_root");
    assert_eq!(rig.registry.live_root(), Some(rig.client.root.clone()));
    rig.registry.set_live_outcomes(None);
    assert_eq!(rig.registry.live_root(), None);
}
