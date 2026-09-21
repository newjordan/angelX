use super::*;
use serde_json::json;
use std::io::Write;

struct TempDir(std::path::PathBuf);
impl TempDir {
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn tempdir() -> std::io::Result<TempDir> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "angel-research-journal-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&path)?;
    Ok(TempDir(path))
}

fn observation(root: &Path, id: &str, links: &[&str], status: &str) -> serde_json::Value {
    json!({"project_root": root, "id": id, "experiment_id": "run", "kind": "measurement",
        "status": status, "title": id, "summary": "Development score 0.844071",
        "detail": "Full public corpus; official result not submitted", "links": links,
        "timestamp": "2026-09-05T02:30:00Z"})
}

fn envelope(sequence: u64, previous: &str, payload: serde_json::Value) -> String {
    let payload_json = payload.to_string();
    let hash = crate::knowledge::cut::sha256_hex(
        format!("{sequence}\n{previous}\n{payload_json}").as_bytes(),
    );
    format!(
        "{}\n",
        json!({"schema": "angel.research-event/v1", "sequence": sequence,
        "previous_sha256": previous, "payload_json": payload_json, "sha256": hash})
    )
}

fn path(root: &Path) -> std::path::PathBuf {
    let dir = root.join(".angelX/research");
    fs::create_dir_all(&dir).unwrap();
    dir.join("experiments.jsonl")
}

#[test]
fn partial_writes_wait_then_project_without_inventing_verification() {
    let root = tempdir().unwrap();
    let path = path(root.path());
    let event = envelope(
        1,
        &"0".repeat(64),
        observation(root.path(), "baseline", &[], "completed"),
    );
    fs::write(&path, &event[..event.len() - 1]).unwrap();
    let mut journal = Journal::default();
    assert!(journal.refresh(root.path()).is_empty());
    assert!(journal.issue.is_none());
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"\n")
        .unwrap();
    let entries = journal.refresh(root.path());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].state, State::Recorded);
    assert!(entries[0].detail.contains("not a native verification"));
    assert_eq!(journal.refresh(root.path()).len(), 1);
}

#[test]
fn replacement_resets_identity_and_bad_chain_stops_at_last_good_record() {
    let root = tempdir().unwrap();
    let path = path(root.path());
    let event = envelope(
        1,
        &"0".repeat(64),
        observation(root.path(), "old", &[], "running"),
    );
    fs::write(&path, event).unwrap();
    let mut journal = Journal::default();
    assert_eq!(journal.refresh(root.path())[0].id, "experiment:old");
    let replacement = path.with_extension("new");
    let event = envelope(
        1,
        &"0".repeat(64),
        observation(root.path(), "new", &[], "completed"),
    );
    fs::write(&replacement, &event).unwrap();
    fs::rename(replacement, &path).unwrap();
    assert_eq!(journal.refresh(root.path())[0].id, "experiment:new");
    let wrong = envelope(
        2,
        &"0".repeat(64),
        observation(root.path(), "bad", &[], "completed"),
    );
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(wrong.as_bytes())
        .unwrap();
    assert_eq!(journal.refresh(root.path()).len(), 1);
    assert_eq!(
        journal.issue,
        Some("research journal continuity check failed")
    );
}

#[test]
fn cross_workspace_records_and_unknown_statuses_are_not_projected() {
    let root = tempdir().unwrap();
    let path = path(root.path());
    let event = envelope(
        1,
        &"0".repeat(64),
        observation(Path::new("/another-project"), "other", &[], "completed"),
    );
    fs::write(&path, event).unwrap();
    let mut journal = Journal::default();
    assert!(journal.refresh(root.path()).is_empty());
    assert!(journal.issue.unwrap().contains("another workspace"));
    let event = envelope(
        1,
        &"0".repeat(64),
        observation(root.path(), "other", &[], "verified"),
    );
    fs::write(&path, event).unwrap();
    let mut journal = Journal::default();
    assert!(journal.refresh(root.path()).is_empty());
    assert_eq!(journal.issue, Some("invalid research observation"));
}

#[test]
fn dependencies_gain_reverse_navigation_and_updates_keep_stable_ids() {
    let root = tempdir().unwrap();
    let path = path(root.path());
    let first = envelope(
        1,
        &"0".repeat(64),
        observation(root.path(), "baseline", &[], "completed"),
    );
    let hash = serde_json::from_str::<serde_json::Value>(&first).unwrap()["sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let second = envelope(
        2,
        &hash,
        observation(root.path(), "evaluation", &["baseline"], "running"),
    );
    let hash = serde_json::from_str::<serde_json::Value>(&second).unwrap()["sha256"]
        .as_str()
        .unwrap()
        .to_string();
    let third = envelope(
        3,
        &hash,
        observation(root.path(), "evaluation", &["baseline"], "completed"),
    );
    fs::write(path, format!("{first}{second}{third}")).unwrap();
    let mut journal = Journal::default();
    let entries = journal.refresh(root.path());
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].state, State::Recorded);
    assert_eq!(entries[0].links[0].0, "experiment:evaluation");
    assert_eq!(entries[1].links[0].0, "experiment:baseline");
    assert!(entries[0].links[0].1.starts_with("Consumer · "));
}

#[cfg(unix)]
#[test]
fn journal_symlink_is_rejected() {
    let root = tempdir().unwrap();
    let path = path(root.path());
    std::os::unix::fs::symlink("/dev/null", path).unwrap();
    let mut journal = Journal::default();
    assert!(journal.refresh(root.path()).is_empty());
    assert!(journal.issue.is_some());
}

#[test]
fn stale_running_observations_become_inconclusive_without_changing_receipts() {
    let root = tempdir().unwrap();
    let path = path(root.path());
    let mut value = observation(root.path(), "worker", &[], "running");
    value["observed_unix_ms"] = json!(1);
    fs::write(path, envelope(1, &"0".repeat(64), value)).unwrap();
    let mut journal = Journal::default();
    assert_eq!(journal.refresh(root.path())[0].state, State::Inconclusive);
    assert_eq!(journal.records[0].state, State::Running);
    assert!(journal.issue.is_none());
}

#[test]
#[ignore = "explicit local capture of real experiment observations, not a synthetic benchmark"]
fn capture_real_experiment_journal() {
    use crate::drive::research_workspace::{Action, Lens, Workspace, view};
    use ratatui::{Terminal, backend::TestBackend};
    let path = std::env::var("RESEARCH_FIXTURE_WORKSPACE").expect("workspace required");
    let output = std::env::var("RESEARCH_PREVIEW_OUTPUT").expect("output prefix required");
    let mut workspace = Workspace::default();
    let mut entries = workspace.journal.refresh(Path::new(&path));
    // A real cockpit refreshes incrementally. Drain this bounded capture input
    // across the same refresh API before taking its static screenshots.
    for _ in 0..=MAX_FILE / READ_BUDGET {
        if workspace.journal.issue.is_some()
            || workspace
                .journal
                .stamp
                .is_none_or(|stamp| workspace.journal.offset >= stamp.0)
        {
            break;
        }
        entries = workspace.journal.refresh(Path::new(&path));
    }
    assert!(
        workspace.journal.issue.is_none(),
        "{:?}",
        workspace.journal.issue
    );
    assert!(entries.iter().any(|entry| entry.measurement.is_some()));
    workspace.project(entries);
    for (name, lens) in [
        ("story", Lens::Story),
        ("ledger", Lens::Ledger),
        ("flow", Lens::Flow),
    ] {
        workspace.action(Action::Lens(lens));
        workspace.offset = 0;
        workspace.reveal_selection = false;
        let mut terminal = Terminal::new(TestBackend::new(110, 36)).unwrap();
        terminal
            .draw(|frame| {
                view::render(
                    frame,
                    &mut workspace,
                    frame.area(),
                    "REAL EXPERIMENT RECEIPTS",
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let screen = buffer
            .content
            .chunks(110)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(screen.contains(if lens == Lens::Flow {
            "RESEARCH LINEAGE"
        } else {
            "RESEARCH PROGRESS"
        }));
        fs::write(format!("{output}-{name}.txt"), screen).unwrap();
    }
}
