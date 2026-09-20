use super::*;
use ratatui::crossterm::event::KeyEvent;

#[test]
fn tutor_question_preserves_work_cursor_and_selection_on_cancel() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.input = "unfinished research λ draft".into();
    app.cursor = 12;
    app.composer_selection_anchor = Some(3);
    app.draft_current_lesson_for_tutor();
    assert!(app.input.is_empty());
    assert!(app.tutor_draft.is_some());
    app.input = "How does a reward model learn?".into();
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.input, "unfinished research λ draft");
    assert_eq!(app.cursor, 12);
    assert_eq!(app.composer_selection_anchor, Some(3));
    assert!(app.tutor_draft.is_none());
    assert!(app.history.is_empty());
    assert!(app.thinking.is_none());
}

#[test]
fn tutor_empty_question_and_busy_agent_preserve_both_drafts() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.input = "work in progress".into();
    app.draft_current_lesson_for_tutor();
    app.submit();
    assert!(app.pending_turn.is_none());
    assert_eq!(
        app.tutor_draft.as_ref().unwrap().saved_input,
        "work in progress"
    );
    app.input = "Explain policy gradients".into();
    arm_turn(&mut app, Vec::new(), None);
    let cancel = Arc::clone(&app.thinking.as_ref().unwrap().cancel);
    app.submit();
    assert_eq!(app.input, "Explain policy gradients");
    assert!(
        app.steer_queue.is_empty(),
        "tutor questions must not steer active work"
    );
    assert!(app.pending_turn.is_none());
    assert!(!cancel.load(std::sync::atomic::Ordering::Relaxed));
    app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.input, "work in progress");
    assert!(!cancel.load(std::sync::atomic::Ordering::Relaxed));
}

#[test]
fn tutor_direct_question_routes_to_an_answer_without_fabricated_source_access() {
    struct TeachingClub(Arc<Mutex<Vec<String>>>);
    impl crate::agent::club::Club for TeachingClub {
        fn label(&self) -> &str {
            "teaching-fixture"
        }
        fn respond(&self, prompt: &str) -> Result<String, String> {
            self.0.lock().unwrap().push(prompt.to_string());
            Ok(
                "A policy gradient adjusts an action's probability using its measured return."
                    .into(),
            )
        }
    }
    let _guard = env_lock();
    let _lookup = TestEnvGuard::set("ANGEL_QUICK_LOOKUP", "0");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut app = seed_preview_app();
    app.bag
        .replace_in_hand_club_for_test(Arc::new(TeachingClub(Arc::clone(&requests))));
    app.input = "/ask How do policy gradients use returns?".into();
    app.submit();
    let deadline = Instant::now() + Duration::from_secs(5);
    while (app.pending_turn.is_some() || app.thinking.is_some()) && Instant::now() < deadline {
        app.advance();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(app.thinking.is_none(), "fixture answer did not finish");
    let sent = requests.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains("How do policy gradients use returns?"));
    assert!(sent[0].contains("not retrieved source material"));
    assert!(
        app.messages
            .iter()
            .any(|message| message.text.contains("A policy gradient adjusts"))
    );
    assert!(
        app.history
            .iter()
            .any(|message| message.content.contains("A policy gradient adjusts"))
    );
}

#[test]
fn tutor_sending_a_question_restores_work_and_keeps_loading_context_honest() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.submit_deferral = true;
    app.scryglass
        .queue_lesson_outcome(crate::ui::term::lookup::TestLookupOutcome::Success {
            title: "Vector space",
            summary: "reference text",
            source_url: "https://example.org/v",
        });
    assert!(app.scryglass.begin_lesson("vector".to_string()));
    assert!(app.scryglass.lesson_loading());
    app.input = "resume this λ derivation".into();
    app.cursor = 7;
    app.draft_current_lesson_for_tutor();
    assert_eq!(
        app.tutor_draft.as_ref().unwrap().name,
        app.scryglass.lesson().unwrap().tutor_name()
    );
    app.input = "Why is an eigenvector useful?".into();
    app.submit();
    assert_eq!(app.input, "resume this λ derivation");
    assert_eq!(app.cursor, 7);
    assert!(app.tutor_draft.is_none());
    let request = app
        .pending_turn
        .as_ref()
        .expect("question reaches the ordinary turn route");
    assert!(
        request
            .user_msg
            .content
            .contains("Why is an eigenvector useful?")
    );
    assert!(request.user_msg.content.contains("catalog pointer"));
    assert!(
        !request.user_msg.content.contains("reference text"),
        "unreceived reference was claimed as evidence"
    );
}

#[test]
fn tutor_bare_ask_and_unknown_multiword_question_are_available_without_a_lesson() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.input = "/ask".into();
    app.submit();
    assert!(app.tutor_draft.is_some());
    assert!(app.input.is_empty());
    assert!(app.cancel_tutor_question());
    app.submit_deferral = true;
    app.input = "/ask What might frobnication mean in this document?".into();
    app.submit();
    let pending = app.pending_turn.as_ref().unwrap();
    assert!(pending.user_msg.content.contains("frobnication"));
    assert!(pending.user_msg.content.contains("Explain uncertainty"));
}

#[cfg(test)]
mod copy_regression {
    use super::*;

    #[test]
    fn ordinary_copy_never_requests_a_lesson_or_changes_a_work_draft() {
        let _guard = env_lock();
        let _lookup = TestEnvGuard::set("ANGEL_QUICK_LOOKUP", "0");
        use ratatui::layout::Rect;
        use ratatui::widgets::Paragraph;
        for text in ["Poisson", "reward model", "policy gradient", "λ values"] {
            let mut app = seed_preview_app();
            app.input = "preserve my unfinished research question λ".into();
            let draft = app.input.clone();
            let width = unicode_width::UnicodeWidthStr::width(text) as u16;
            let area = Rect::new(0, 0, width, 1);
            app.panes.clear();
            app.panes.push(mouse::PaneId::Transcript, area);
            app.selection = Some(mouse::Selection::new(mouse::PaneId::Transcript, area, 0, 0));
            app.selection.as_mut().unwrap().extend(width - 1, 0);
            app.copy_requested = true;
            let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
            terminal
                .draw(|frame| {
                    frame.render_widget(Paragraph::new(text), area);
                    draw::render_selection_and_approval_overlays(frame, &mut app);
                })
                .unwrap();
            assert_eq!(
                app.pending_clipboard.as_deref(),
                Some(text),
                "copy must still work"
            );
            assert!(
                app.pending_quick_lookup.is_none(),
                "copying {text:?} must not ask a tutor"
            );
            assert!(
                app.scryglass.lesson().is_none(),
                "copy must not open a lesson"
            );
            assert_eq!(app.input, draft, "copy must preserve the user's work");
            assert!(!app.copy_requested);
        }
    }
}
