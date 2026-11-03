use super::*;

#[test]
fn a_picture_is_made_off_the_callers_thread_and_waited_for() {
    let pipeline = Pipeline::new();
    let caller = std::thread::current().id();
    pipeline.request(
        7,
        Box::new(move || {
            assert_ne!(std::thread::current().id(), caller);
            Some(vec![1, 2, 3])
        }),
    );
    let png = pipeline.wait(7, Duration::from_secs(5)).unwrap();
    assert_eq!(*png, vec![1, 2, 3]);
    assert_eq!(pipeline.latest().unwrap().0, 7);
    // Asking again for the same picture makes nothing new.
    pipeline.request(7, Box::new(|| panic!("already made")));
    assert!(pipeline.wait(7, Duration::ZERO).is_some());
}

#[test]
fn only_the_newest_waiting_request_is_made() {
    let pipeline = Pipeline::new();
    let (hold, release) = std::sync::mpsc::channel::<()>();
    pipeline.request(
        1,
        Box::new(move || {
            let _ = release.recv();
            Some(vec![1])
        }),
    );
    // While 1 is being made, 2 waits and is then replaced by 3.
    std::thread::sleep(Duration::from_millis(20));
    pipeline.request(2, Box::new(|| panic!("replaced before it was made")));
    pipeline.request(3, Box::new(|| Some(vec![3])));
    assert!(
        pipeline.wait(3, Duration::from_millis(10)).is_none(),
        "still busy with 1"
    );
    drop(hold);
    assert_eq!(*pipeline.wait(3, Duration::from_secs(5)).unwrap(), vec![3]);
    assert!(pipeline.wait(2, Duration::from_millis(10)).is_none());
}
