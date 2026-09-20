use super::*;
use crate::knowledge::memory::store::{Drawer, MemoryStore};

struct BlockingStore {
    calls: Arc<AtomicUsize>,
    release: Arc<AtomicBool>,
}

impl MemoryStore for BlockingStore {
    fn deposit(&self, _drawer: &Drawer) -> Result<String, String> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        while !self.release.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(String::new())
    }
    fn search(
        &self,
        _query: &str,
        _limit: usize,
        _wing: Option<&str>,
    ) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn status(&self) -> Result<String, String> {
        Ok(String::new())
    }
}

struct PanickingStore;

impl MemoryStore for PanickingStore {
    fn deposit(&self, _drawer: &Drawer) -> Result<String, String> {
        panic!("fixture palace panic")
    }
    fn search(
        &self,
        _query: &str,
        _limit: usize,
        _wing: Option<&str>,
    ) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn status(&self) -> Result<String, String> {
        Ok(String::new())
    }
}

struct CountingStore(Arc<AtomicUsize>);

impl MemoryStore for CountingStore {
    fn deposit(&self, _drawer: &Drawer) -> Result<String, String> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Ok(String::new())
    }
    fn search(
        &self,
        _query: &str,
        _limit: usize,
        _wing: Option<&str>,
    ) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn status(&self) -> Result<String, String> {
        Ok(String::new())
    }
}

fn drawer(label: &str) -> Drawer {
    Drawer {
        wing: "test".to_string(),
        room: "queue".to_string(),
        content: label.to_string(),
        source: "fixture".to_string(),
    }
}

#[test]
fn palace_write_queue_bounds_a_hung_store() {
    let queue = spawn_memory_write_queue(1);
    let (summary_tx, summary_rx) = mpsc::channel();
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(AtomicBool::new(false));
    let store: Arc<dyn MemoryStore> = Arc::new(BlockingStore {
        calls: Arc::clone(&calls),
        release: Arc::clone(&release),
    });
    let job = |label| {
        let summary_tx = summary_tx.clone();
        MemoryWriteJob::Drawers {
            store: Arc::clone(&store),
            drawers: vec![drawer(label)],
            completion: Some(Box::new(move |summary| {
                let _ = summary_tx.send(summary);
            })),
        }
    };

    assert!(try_enqueue_memory_write(&queue, job("active")));
    let started_deadline = Instant::now() + Duration::from_secs(1);
    while calls.load(Ordering::Acquire) == 0 && Instant::now() < started_deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert!(try_enqueue_memory_write(&queue, job("queued")));
    assert!(!try_enqueue_memory_write(&queue, job("bounded")));
    let rejected = summary_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("full queue must report the dropped batch");
    assert_eq!(rejected.attempted, 1);
    assert_eq!(rejected.filed, 0);
    assert!(
        rejected
            .first_error
            .as_deref()
            .is_some_and(|error| error.contains("queue is full"))
    );

    release.store(true, Ordering::Release);
    let completed_deadline = Instant::now() + Duration::from_secs(1);
    while calls.load(Ordering::Acquire) < 2 && Instant::now() < completed_deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(calls.load(Ordering::Acquire), 2);
}

#[test]
fn palace_write_queue_survives_a_store_panic() {
    let queue = spawn_memory_write_queue(2);
    let completed = Arc::new(AtomicUsize::new(0));
    assert!(
        queue
            .try_send(MemoryWriteJob::Drawers {
                store: Arc::new(PanickingStore),
                drawers: vec![drawer("panic")],
                completion: None,
            })
            .is_ok()
    );
    assert!(
        queue
            .try_send(MemoryWriteJob::Drawers {
                store: Arc::new(CountingStore(Arc::clone(&completed))),
                drawers: vec![drawer("after panic")],
                completion: None,
            })
            .is_ok()
    );

    let deadline = Instant::now() + Duration::from_secs(1);
    while completed.load(Ordering::Acquire) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(completed.load(Ordering::Acquire), 1);
}
