use super::*;
use std::sync::Mutex;

/// A store double that records every deposited drawer.
struct RecordingStore(Mutex<Vec<Drawer>>);
impl MemoryStore for RecordingStore {
    fn deposit(&self, d: &Drawer) -> Result<String, String> {
        self.0.lock().unwrap().push(d.clone());
        Ok(String::new())
    }
    fn search(&self, _q: &str, _l: usize, _w: Option<&str>) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn status(&self) -> Result<String, String> {
        Ok(String::new())
    }
}

fn report(body: &str) -> Report {
    Report {
        wing: "angelX".into(),
        topic: "Findings".into(),
        body: body.into(),
        source: "moa".into(),
    }
}

#[test]
fn pass_through_files_one_drawer() {
    let store = Arc::new(RecordingStore(Mutex::new(Vec::new())));
    let lib = Librarian::new(store.clone());
    assert!(lib.file(&report("found a deadlock in run_turn")));
    let filed = store.0.lock().unwrap();
    assert_eq!(filed.len(), 1);
    assert_eq!(filed[0].room, "Findings");
    assert_eq!(filed[0].content, "found a deadlock in run_turn");
}

#[test]
fn empty_report_files_nothing() {
    let store = Arc::new(RecordingStore(Mutex::new(Vec::new())));
    let lib = Librarian::new(store.clone());
    assert!(!lib.file(&report("   ")));
    assert!(store.0.lock().unwrap().is_empty());
}

/// A store that always rejects deposits — to prove `file` reports failure when
/// nothing actually lands.
struct FailingStore;
impl MemoryStore for FailingStore {
    fn deposit(&self, _d: &Drawer) -> Result<String, String> {
        Err("backend down".to_string())
    }
    fn search(&self, _q: &str, _l: usize, _w: Option<&str>) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn status(&self) -> Result<String, String> {
        Ok(String::new())
    }
}

#[test]
fn file_returns_false_when_the_store_rejects_every_deposit() {
    let lib = Librarian::new(Arc::new(FailingStore));
    // A perfectly good report, but the store can't take it → nothing filed.
    assert!(!lib.file(&report("a real finding worth keeping")));
}

#[test]
fn curate_trims_body_and_propagates_provenance() {
    let store = Arc::new(RecordingStore(Mutex::new(Vec::new())));
    let lib = Librarian::new(store.clone());
    let r = Report {
        wing: "myproj".into(),
        topic: "Decisions".into(),
        body: "\n  chose the lock-free queue  \n".into(),
        source: "sess-9/moa".into(),
    };
    assert!(lib.file(&r));
    let filed = store.0.lock().unwrap();
    assert_eq!(filed.len(), 1);
    let d = &filed[0];
    assert_eq!(d.content, "chose the lock-free queue", "body is trimmed");
    assert_eq!(d.wing, "myproj");
    assert_eq!(d.room, "Decisions", "topic becomes the drawer room");
    assert_eq!(d.source, "sess-9/moa");
}
