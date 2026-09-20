use super::*;

#[test]
fn input_keeps_every_byte_while_consumer_is_backpressured() {
    let _guard = crate::tests::env_lock();
    let count = 16384;
    let (done, drained) = mpsc::channel();
    let mut position = 0;
    let input = TerminalInput::spawn(move |wait| {
        if position == count {
            let _ = done.send(());
            std::thread::sleep(wait);
            return Ok(None);
        }
        let key = event::KeyEvent::new(
            event::KeyCode::Char((b'a' + (position % 26) as u8) as char),
            event::KeyModifiers::NONE,
        );
        position += 1;
        Ok(Some(Event::Key(key)))
    })
    .unwrap();
    // No consumption until the input worker has drained the entire burst.
    drained.recv_timeout(Duration::from_secs(2)).unwrap();
    for i in 0..count {
        let expected = Event::Key(event::KeyEvent::new(
            event::KeyCode::Char((b'a' + (i % 26) as u8) as char),
            event::KeyModifiers::NONE,
        ));
        assert_eq!(input.next(Duration::ZERO).unwrap(), Some(expected));
    }
    assert!(input.next(Duration::ZERO).unwrap().is_none());
}
