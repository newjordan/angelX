use super::*;

struct PanickingTeacher;

impl Club for PanickingTeacher {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        panic!("synthetic teacher failure");
    }

    fn label(&self) -> &str {
        "panicking-teacher"
    }
}

#[test]
fn clerk_worker_inherits_the_launching_book_generation() {
    use crate::agent::club::{ChatMsg, ChatRole, ClubReply, ToolCall, ToolDef};
    use crate::agent::harness::book::{Route, catalog};

    struct LedgerTeacher(Arc<Mutex<Option<String>>>);
    impl Club for LedgerTeacher {
        fn label(&self) -> &str {
            "ledger-teacher"
        }
        fn respond(&self, _: &str) -> Result<String, String> {
            Err("fixture requires connected chat".into())
        }
        fn chat(&self, messages: &[ChatMsg], _: &[ToolDef]) -> Result<ClubReply, String> {
            if let Some(result) = messages
                .iter()
                .find(|message| message.role == ChatRole::Tool)
            {
                *self.0.lock().unwrap() = Some(result.content.to_string());
                return Ok(ClubReply::Text(
                    r#"{"schema":"angel-atlas-clerk/v1","action":"abstain","candidates":[]}"#
                        .into(),
                ));
            }
            Ok(ClubReply::Calls(vec![ToolCall {
                id: "clerk-book".into(),
                name: "read_file".into(),
                args: serde_json::json!({"path":"ledger://⠣⠁⠁"}),
            }]))
        }
    }

    let root = crate::tests::TestGitWorkspace::new("atlas-clerk-book");
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let atlas =
        crate::knowledge::atlas::AtlasService::open_in(&workspace, root.path().join("atlas"));
    atlas.enqueue_harvest("task", "answer", &[], &[]).unwrap();
    let worker = AtlasClerkWorker::shared(Arc::clone(&atlas));
    let observed = Arc::new(Mutex::new(None));
    let mut generation = catalog::Catalog::baseline();
    generation.set_test_page(Route::new('⠣', '⠁'), 0, "atlas pinned catalog page");
    let scope = catalog::enter(Arc::new(generation));
    worker.tick(
        true,
        || {
            Some(ClerkRoute {
                route_id: RouteId::chat("teacher", "ledger-teacher", "ledger-teacher", None),
                model_revision: ModelRevision::chat("ledger-teacher"),
                resource_group: "atlas-book-test".into(),
                club: Arc::new(LedgerTeacher(Arc::clone(&observed))),
            })
        },
        Arc::new(BackplaneRegistry::default()),
    );
    drop(scope);
    for _ in 0..200 {
        if worker.status().health != ClerkHealth::Running {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        observed
            .lock()
            .unwrap()
            .as_deref()
            .unwrap_or("")
            .contains("atlas pinned catalog page")
    );
    assert_eq!(worker.status().health, ClerkHealth::Idle);
}

#[test]
fn unavailable_teacher_leaves_persisted_work_queued() {
    let root = std::env::temp_dir().join(format!(
        "angel-atlas-clerk-unavailable-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let atlas = crate::knowledge::atlas::AtlasService::open_in(&workspace, root.clone());
    atlas.enqueue_harvest("task", "answer", &[], &[]).unwrap();
    let worker = AtlasClerkWorker::shared(Arc::clone(&atlas));
    worker.tick(true, || None, Arc::new(BackplaneRegistry::default()));
    let status = worker.status();
    assert_eq!(status.health, ClerkHealth::WaitingForRoute);
    assert_eq!(status.queue_depth, 1);
    assert!(atlas.next_harvest().is_some());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn panicking_teacher_releases_lease_and_recovers_scheduler_state() {
    let root = std::env::temp_dir().join(format!(
        "angel-atlas-clerk-panic-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let atlas = crate::knowledge::atlas::AtlasService::open_in(&workspace, root.clone());
    atlas.enqueue_harvest("task", "answer", &[], &[]).unwrap();
    let worker = AtlasClerkWorker::shared(Arc::clone(&atlas));
    let registry = Arc::new(BackplaneRegistry::default());

    worker.tick(
        true,
        || {
            Some(ClerkRoute {
                route_id: RouteId::chat("teacher", "panicking-teacher", "panicking-teacher", None),
                model_revision: ModelRevision::chat("panicking-teacher"),
                resource_group: "teacher-test".to_string(),
                club: Arc::new(PanickingTeacher),
            })
        },
        Arc::clone(&registry),
    );

    let mut status = worker.status();
    for _ in 0..100 {
        if status.health != ClerkHealth::Running {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        status = worker.status();
    }
    assert_eq!(status.health, ClerkHealth::Degraded);
    assert_eq!(
        status.last_result.as_deref(),
        Some("atlas clerk worker panicked")
    );
    assert!(registry.leases().is_empty());
    assert!(atlas.next_harvest().is_some());
    let _ = std::fs::remove_dir_all(root);
}
