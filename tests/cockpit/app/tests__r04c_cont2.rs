use super::*;
use ratatui::crossterm::event;

fn fifo(events: Vec<Event>) -> crate::input::TerminalInput {
    let mut events = events.into_iter();
    let (done, ready) = mpsc::channel();
    let mut done = Some(done);
    let input = crate::input::TerminalInput::spawn(move |wait| {
        if let Some(event) = events.next() {
            Ok(Some(event))
        } else {
            if let Some(done) = done.take() {
                let _ = done.send(());
            }
            std::thread::sleep(wait);
            Ok(None)
        }
    })
    .unwrap();
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    input
}

#[test]
fn r04c_cont2_large_stream_phase_costs() {
    large_stream_phase_costs(false);
}

#[test]
fn r04c_cont2_large_stream_successful_cancel_costs() {
    large_stream_phase_costs(true);
}

fn large_stream_phase_costs(successful: bool) {
    let _guard = env_lock();
    let chunk = "ordinary prose with words and a newline.\n".repeat(100);
    let count = (6 * 1024 * 1024usize).div_ceil(chunk.len());
    let target = chunk.len() * count;
    let events = (0..count)
        .map(|_| harness::TurnEvent::Token(chunk.clone()))
        .collect();
    let (mut app, tx) = seed_live_streaming_app(events);
    let atlas_dir = std::env::temp_dir().join(format!(
        "r04c-cont2-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let workspace = atlas_dir.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    if successful {
        app.atlas = crate::atlas::AtlasService::open_in(&workspace, atlas_dir.join("atlas"));
    }
    if let Some(path) = std::env::var_os("ANGEL_FRAME_TIMING_LOG") {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(file, "# fixture=incremental_app successful={successful} (synthetic input/advance; no terminal draw/settle)").unwrap();
    }
    let mut timing = crate::frame_timing::FrameTiming::from_env();
    let mut worst = Duration::ZERO;
    let mut frames = 0;
    let (keys, receiver) = mpsc::channel();
    let input = crate::input::TerminalInput::spawn(move |wait| match receiver.recv_timeout(wait) {
        Ok(event) => Ok(Some(event)),
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok(None),
    })
    .unwrap();
    while app.partial.len() < target {
        keys.send(Event::Key(event::KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::NONE,
        )))
        .unwrap();
        let first = input.next(Duration::from_secs(2)).unwrap();
        let started = Instant::now();
        crate::apply_terminal_input(&mut app, &input, first).unwrap();
        let input_us = started.elapsed().as_micros();
        assert_eq!(
            app.input.len(),
            frames + 1,
            "enqueued key must apply in this frame"
        );
        let started = Instant::now();
        app.advance();
        let advance = started.elapsed();
        worst = worst.max(advance);
        let draw_started = Instant::now();
        if let Some(timing) = &mut timing {
            timing.completed(
                draw_started,
                Instant::now(),
                crate::frame_timing::Phases {
                    input_us,
                    advance_us: advance.as_micros(),
                    ..Default::default()
                },
            );
        }
        frames += 1;
        assert!(frames <= count, "stream stopped making progress");
    }
    let started = Instant::now();
    app.on_key(event::KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let cancel = started.elapsed();
    if successful {
        let reply = app.partial.clone();
        tx.send(Ok((
            vec![ChatMsg::assistant(reply.clone())],
            reply,
            crate::club::RouteIdentity {
                driver: "practice".into(),
                model: None,
                reasoning_effort: None,
            },
            crate::harness::TurnStopReason::Interrupt,
        )))
        .unwrap();
    } else {
        tx.send(Err("scripted cancellation".into())).unwrap();
    }
    let started = Instant::now();
    app.advance();
    let end = started.elapsed();
    if let Some(timing) = &mut timing {
        let draw_started = Instant::now();
        timing.completed(
            draw_started,
            Instant::now(),
            crate::frame_timing::Phases {
                input_us: cancel.as_micros(),
                advance_us: end.as_micros(),
                ..Default::default()
            },
        );
    }
    assert!(app.thinking.is_none());
    assert!(
        app.messages
            .iter()
            .any(|m| matches!(m.role, Role::Angel) && m.text.len() == target)
    );
    for _ in 0..3 {
        let start = Instant::now();
        app.advance();
        let duration = start.elapsed();
        assert!(
            duration < Duration::from_millis(150),
            "post-turn {duration:?}"
        );
    }
    if successful {
        // A large harvest may outlive the bounded tick wait; later ticks collect
        // its acknowledgement without blocking, and the write is never dropped.
        let ack_deadline = Instant::now() + Duration::from_secs(10);
        while app
            .atlas_harvester
            .as_ref()
            .is_some_and(|harvester| harvester.outstanding() > 0)
        {
            assert!(
                Instant::now() < ack_deadline,
                "harvest acknowledgement never arrived"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(app.atlas_harvester.take());
        assert!(
            app.atlas.next_harvest().is_some(),
            "accepted harvest is durable after worker drain"
        );
    }
    eprintln!(
        "r04c_cont2 successful={successful} bytes={target} frames={frames} max_advance_us={} cancel_us={} turn_end_us={}",
        worst.as_micros(),
        cancel.as_micros(),
        end.as_micros()
    );
    assert!(worst < Duration::from_millis(150), "advance max {worst:?}");
    assert!(cancel < Duration::from_millis(150), "cancel {cancel:?}");
    assert!(end < Duration::from_millis(150), "turn end {end:?}");
    drop(app);
    std::fs::remove_dir_all(atlas_dir).unwrap();
}

#[test]
fn r04c_cont2_startup_literal_clear_literal_enter_is_fifo() {
    let _guard = env_lock();
    let first = "R04R delayed first literal burst";
    let second = "R04R second literal burst";
    let literal = |s: &str| {
        s.chars()
            .map(|c| Event::Key(event::KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)))
            .collect::<Vec<_>>()
    };
    let mut events = literal(first);
    events.push(Event::Key(event::KeyEvent::new(
        KeyCode::Char('u'),
        KeyModifiers::CONTROL,
    )));
    events.extend(literal(second));
    events.push(Event::Key(event::KeyEvent::new(
        KeyCode::Enter,
        KeyModifiers::NONE,
    )));
    let input = fifo(events);
    let mut app = seed_preview_app();
    app.submit_deferral = true;
    let started = Instant::now();
    let next = input.next(Duration::ZERO).unwrap();
    crate::apply_terminal_input(&mut app, &input, next).unwrap();
    let elapsed = started.elapsed();
    eprintln!("startup queued_fifo_apply_us={}", elapsed.as_micros());
    assert!(elapsed < Duration::from_millis(100));
    assert!(app.input.is_empty());
    assert!(app.pending_turn.is_some());
    let messages: Vec<_> = app
        .messages
        .iter()
        .filter(|m| matches!(m.role, Role::User))
        .collect();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].text.as_ref(), second);
}

#[test]
fn r04c_cont2_cancel_discards_unconsumed_stream_without_blocking_input() {
    let _guard = env_lock();
    let events = (0..100_000)
        .map(|_| harness::TurnEvent::Token("queued prose".into()))
        .collect();
    let (mut app, tx) = seed_live_streaming_app(events);
    app.on_key(event::KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    tx.send(Err("scripted cancellation".into())).unwrap();
    let started = Instant::now();
    app.advance();
    let elapsed = started.elapsed();
    assert!(app.partial.is_empty());
    assert!(app.thinking.is_none());
    app.on_key(event::KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert_eq!(app.input, "x");
    eprintln!(
        "cancel queued_events=100000 advance_us={}",
        elapsed.as_micros()
    );
    assert!(elapsed < Duration::from_millis(150));
}
