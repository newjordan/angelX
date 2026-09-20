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
fn unavailable_teacher_leaves_persisted_work_queued() {
    let root = std::env::temp_dir().join(format!(
        "angel-atlas-clerk-unavailable-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let atlas = crate::atlas::AtlasService::open_in(&workspace, root.clone());
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
    let atlas = crate::atlas::AtlasService::open_in(&workspace, root.clone());
    atlas.enqueue_harvest("task", "answer", &[], &[]).unwrap();
    let worker = AtlasClerkWorker::shared(Arc::clone(&atlas));
    let registry = Arc::new(BackplaneRegistry::default());

    worker.tick(
        true,
        || {
            Some(ClerkRoute {
                route_id: RouteId::chat("teacher", "panicking-teacher", None),
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
