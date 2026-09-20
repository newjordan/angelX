use super::*;

#[test]
fn completed_waits_release_claims_before_the_handle_is_dropped() {
    let _lock = crate::tests::env_lock();
    for kind in ["wait", "try_wait", "output"] {
        let mut child = Command::new("sh")
            .args(["-c", "exit 23"])
            .spawn_owned()
            .unwrap();
        let pid = child.id();
        assert!(claims().lock().unwrap().direct.contains_key(&pid));
        match kind {
            "wait" => assert_eq!(child.wait().unwrap().code(), Some(23)),
            "try_wait" => {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                loop {
                    if let Some(status) = child.try_wait().unwrap() {
                        assert_eq!(status.code(), Some(23));
                        break;
                    }
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
            _ => {
                assert_eq!(child.wait_with_output().unwrap().status.code(), Some(23));
                assert!(!claims().lock().unwrap().direct.contains_key(&pid));
                continue;
            }
        }
        assert!(!claims().lock().unwrap().direct.contains_key(&pid));
        assert_eq!(child.try_wait().unwrap().unwrap().code(), Some(23));
    }
}
