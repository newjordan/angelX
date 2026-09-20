//! Process-level capture/export contract using only the loopback scripted provider.
use serde_json::Value;
use std::process::Command;

mod tests {
    pub fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|error| error.into_inner())
    }
}

fn episode(mode: &str) -> Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let output = Command::new("python3")
        .args(["-c", r#"
import importlib.util, json, os, pathlib, subprocess, sys, tempfile
root, binary, mode = sys.argv[1:]
spec = importlib.util.spec_from_file_location('seed', pathlib.Path(root)/'scripts/gen-harness-rollout-seed.py')
gen = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gen)
workspace = pathlib.Path(tempfile.mkdtemp(prefix='rollout-contract-'))
store = workspace/'rollouts'
if mode in ('blocked', 'required'):
    store.write_text('not a writable directory')
env = dict(os.environ)
env.update(HOME=str(workspace/'home'), ANGEL_HARNESS_ROLLOUT_DIR=str(store),
    ANGEL_HARNESS_ROLLOUT_REQUIRED='1' if mode == 'required' else '0',
    ANGEL_TASK_RECON='0', ANGEL_PROJECT_DOC='0', ANGEL_TASK_EVENT_LOG='0',
    ANGEL_DRIVER='openrouter', ANGEL_API_CLUBS='openrouter',
    ANGEL_OPENROUTER_KEY='offline-seed-key', OPENROUTER_API_KEY='offline-seed-key',
    ANGEL_OPENROUTER_MODEL='offline-seed-model', ANGEL_HTTP_RETRIES='0', ANGEL_PROVIDER_RETRIES='0')
with gen.ScriptedServer([gen._sse_text('done')]) as server:
    if mode == 'late':
        original = server.server.RequestHandlerClass.do_POST
        def fail_store(handler):
            store.rename(workspace/'saved-rollouts')
            store.write_text('store became unavailable after startup')
            original(handler)
        server.server.RequestHandlerClass.do_POST = fail_store
    env['ANGEL_OPENROUTER_URL'] = server.url
    task = subprocess.run([binary, '--task-json', '--workspace', str(workspace), '--rollout', 'local',
        '--task-id', 'persistence-contract', '--run-id', 'persistence-contract', 'reply done'],
        env=env, capture_output=True, text=True)
    requests = len(server.requests)
tree = sorted(str(p.relative_to(workspace)) for p in workspace.rglob('*') if p.is_relative_to(store))
export_path = workspace/'corpus.json'
export = subprocess.run([binary, '--export-harness-rollout', '--workspace', str(workspace), '--all', str(export_path)],
    env=env, capture_output=True, text=True)
envelope = json.loads(task.stdout)
audit = None
if mode == 'healthy':
    audited = subprocess.run([binary, '--audit-harness-rollout', '--workspace', str(workspace),
        envelope['rollout_id'], '-'], env=env, capture_output=True, text=True)
    assert audited.returncode == 0, audited.stderr
    audit = json.loads(audited.stdout)
print(json.dumps(dict(workspace=str(workspace), task_exit=task.returncode, envelope=envelope, audit=audit,
    stderr=task.stderr, requests=requests, tree=tree, export_exit=export.returncode,
    export_stderr=export.stderr, corpus=json.loads(export_path.read_text()) if export_path.exists() else None)))
"#])
        .arg(root)
        .arg(env!("CARGO_BIN_EXE_angel"))
        .arg(mode)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("TMPDIR", std::env::temp_dir())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: Value = serde_json::from_slice(&output.stdout).unwrap();
    println!("{}", serde_json::to_string_pretty(&receipt).unwrap());
    receipt
}

#[test]
fn harness_rollout_persists_and_exports_scripted_task() {
    let _guard = crate::tests::env_lock();
    let receipt = episode("healthy");
    assert_eq!(receipt["task_exit"], 0);
    assert_eq!(receipt["envelope"]["stop_reason"], "answer");
    assert_eq!(receipt["requests"], 1);
    assert!(
        receipt["tree"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.as_str().unwrap().ends_with("/manifest.json")),
        "no finalized manifest"
    );
    assert!(receipt["envelope"]["rollout_id"].is_string());
    assert_eq!(receipt["export_exit"], 0);
    assert!(
        receipt["audit"].is_object(),
        "sealed record exports an audited receipt"
    );
    // Headless labels belong to an external evaluator. The exporter must
    // discover and audit the sealed record without inventing a reward.
    assert_eq!(receipt["corpus"]["audit"]["scanned"], 1);
    assert_eq!(receipt["corpus"]["audit"]["audited"], 1);
    assert_eq!(receipt["corpus"]["audit"]["by_status"]["finalized"], 1);
    assert_eq!(
        receipt["corpus"]["audit"]["by_exclusion_reason"]["missing_reward"],
        1
    );
}

#[test]
fn harness_rollout_unwritable_store_reports_capture_error() {
    let _guard = crate::tests::env_lock();
    let receipt = episode("blocked");
    // Optional capture must not stop the run, but its failure must be visible
    // even with ordinary task-event logging disabled.
    assert_eq!(receipt["envelope"]["stop_reason"], "answer");
    assert_eq!(receipt["requests"], 1);
    assert!(
        receipt["stderr"]
            .as_str()
            .unwrap()
            .contains("rollout capture")
    );
    assert!(
        !receipt["envelope"]["rollout_capture_errors"]
            .as_array()
            .expect("structured capture errors")
            .is_empty()
    );
    assert!(receipt["envelope"]["rollout_id"].is_null());
}

#[test]
fn harness_rollout_required_store_failure_is_visible_without_event_logging() {
    let _guard = crate::tests::env_lock();
    let receipt = episode("required");
    assert_eq!(receipt["task_exit"], 1);
    assert_eq!(receipt["envelope"]["stop_reason"], "capture_failure");
    assert_eq!(receipt["requests"], 0);
    assert!(
        receipt["envelope"]["error"]
            .as_str()
            .unwrap()
            .contains("rollout capture")
    );
    assert!(
        receipt["stderr"]
            .as_str()
            .unwrap()
            .contains("rollout capture")
    );
}

#[test]
fn harness_rollout_late_store_failure_is_visible_without_stopping_the_turn() {
    let _guard = crate::tests::env_lock();
    let receipt = episode("late");
    assert_eq!(receipt["envelope"]["stop_reason"], "answer");
    assert_eq!(receipt["requests"], 1);
    assert!(
        receipt["stderr"]
            .as_str()
            .unwrap()
            .contains("rollout capture")
    );
    assert!(
        !receipt["envelope"]["rollout_capture_errors"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
