use super::*;
#[test]
fn spawn_default_is_a_bounded_900_second_clock_and_explicit_seconds_are_exact() {
    let _guard = crate::tests::env_lock();
    let _unset = crate::tests::TestEnvGuard::unset("ANGEL_SPAWN_TIMEOUT");
    let default = configured_formation_timeout(&serde_json::json!({}));
    assert_eq!(default.as_secs(), 900);
    let _cap = crate::tests::TestEnvGuard::set("ANGEL_SPAWN_TIMEOUT", "7200");
    assert_eq!(
        configured_formation_timeout(&serde_json::json!({})).as_secs(),
        7200
    );
    assert_eq!(
        configured_formation_timeout(&serde_json::json!({"timeout_secs": 1})).as_secs(),
        1
    );
    assert!(configured_formation_timeout(&serde_json::json!({"timeout_secs": 0})).is_zero());
}

struct ParentCancelSeat {
    started: std::sync::mpsc::Sender<()>,
    observed: std::sync::mpsc::Sender<bool>,
    release: Arc<std::sync::atomic::AtomicBool>,
}

impl crate::agent::club::Club for ParentCancelSeat {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Err("unexpected non-streaming seat request".into())
    }

    fn label(&self) -> &str {
        "parent-cancel-seat"
    }

    fn chat_streaming(
        &self,
        _messages: &[crate::agent::club::ChatMsg],
        _tools: &[crate::agent::harness::ToolDef],
        cancel: &std::sync::atomic::AtomicBool,
        _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
    ) -> Result<crate::agent::club::ClubReply, String> {
        let _ = self.started.send(());
        while !cancel.load(std::sync::atomic::Ordering::Acquire)
            && !self.release.load(std::sync::atomic::Ordering::Acquire)
        {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let observed = cancel.load(std::sync::atomic::Ordering::Acquire);
        let _ = self.observed.send(observed);
        Err(if observed {
            "seat cancelled by formation".into()
        } else {
            "test fixture released after parent cancellation was not forwarded".into()
        })
    }
}

#[test]
fn parent_turn_cancellation_stops_formation_collector_and_cancels_seat() {
    let _guard = crate::tests::env_lock();
    let workspace = std::env::temp_dir().join(format!(
        "angel_spawn_parent_cancel_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&workspace);
    std::fs::create_dir_all(&workspace).unwrap();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (observed_tx, observed_rx) = std::sync::mpsc::channel();
    let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let club: Arc<dyn crate::agent::club::Club> = Arc::new(ParentCancelSeat {
        started: started_tx,
        observed: observed_tx,
        release: Arc::clone(&release),
    });
    let tool = SpawnTool::new(workspace.clone(), Some(Arc::clone(&club)), vec![club]);
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let result = crate::agent::harness::Tool::call_with_cancel(
            &tool,
            &serde_json::json!({
                "task": "wait for parent cancellation",
                "formation": "solo",
                "n": 1,
                "timeout_secs": 0,
                "tools": "none"
            }),
            Some(&worker_cancel),
        );
        let _ = result_tx.send(result);
    });

    let started = started_rx.recv_timeout(std::time::Duration::from_secs(5)).is_ok();
    cancel.store(true, std::sync::atomic::Ordering::Release);
    let observed = match observed_rx.recv_timeout(std::time::Duration::from_secs(2)) {
        Ok(observed) => Some(observed),
        Err(_) => {
            release.store(true, std::sync::atomic::Ordering::Release);
            observed_rx.recv_timeout(std::time::Duration::from_secs(2)).ok()
        }
    };
    let result = match result_rx.recv_timeout(std::time::Duration::from_secs(2)) {
        Ok(result) => Some(result),
        Err(_) => {
            release.store(true, std::sync::atomic::Ordering::Release);
            result_rx.recv_timeout(std::time::Duration::from_secs(2)).ok()
        }
    };
    let worker_deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !worker.is_finished() && std::time::Instant::now() < worker_deadline {
        std::thread::yield_now();
    }
    let worker_finished = worker.is_finished();
    if worker_finished {
        worker.join().unwrap();
    } else {
        drop(worker);
    }

    assert!(started, "formation did not start its seat");
    assert_eq!(observed, Some(true), "the existing seat token must be set");
    let returned_cancel_error = result.as_ref().is_some_and(|result| {
        result
            .as_ref()
            .err()
            .is_some_and(|error| error.contains("formation cancelled"))
    });
    assert!(
        returned_cancel_error,
        "parent cancellation should return the normal formation cancellation error: {result:?}"
    );
    assert!(worker_finished, "formation collector must stop promptly");
    let _ = std::fs::remove_dir_all(workspace);
}
