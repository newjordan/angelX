//! Paste-handling suites (module-breakup: extracted from the `tests.rs`
//! monolith). Bulk insertion at the caret, line-ending normalization,
//! single-shot clipping with truthful guidance, UTF-8-safe caps, the at-cap
//! refusal — and Ctrl-V clipboard ingress (screenshots).
//!
//! Every clipboard suite drives the *production* read path (spawn → bounded
//! read → admission → drain) against a local fake tool, so no suite reads or
//! writes the desktop clipboard.

use super::{render_app_text, seed_preview_app};
use crate::agent::club::{ChatMsg, ChatRole, Media};
use crate::app::control;
use crate::tests::{TestEnvGuard, env_lock};
use crate::ui::clipboard::ClipboardPaste;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn paste_inserts_text_into_composer_without_submitting() {
    let mut app = seed_preview_app();
    app.on_paste("Return ONE complete document\n<canvas id=\"game\">ធ្វើការ");
    assert_eq!(
        app.input,
        "Return ONE complete document\n<canvas id=\"game\">ធ្វើការ"
    );
    assert!(!app.should_quit);
    assert!(app.thinking.is_none());
}

#[test]
fn paste_is_bulk_inserted_at_the_caret_and_normalizes_line_endings() {
    let mut app = seed_preview_app();
    app.input = "ac".to_string();
    app.cursor = 1;
    app.on_paste("b\r\nsecond\rline");
    assert_eq!(app.input, "ab\nsecond\nlinec");
    assert_eq!(app.cursor, "ab\nsecond\nline".chars().count());
    assert!(app.thinking.is_none(), "paste never submits the draft");
}

#[test]
fn oversized_paste_is_clipped_once_with_truthful_recovery_guidance() {
    let mut app = seed_preview_app();
    let payload = "x".repeat(control::MAX_COMPOSER_PASTE_BYTES + 73);

    app.on_paste(&payload);

    assert_eq!(app.input.len(), control::MAX_COMPOSER_PASTE_BYTES);
    assert_eq!(app.cursor, control::MAX_COMPOSER_PASTE_BYTES);
    let receipt = &app.messages.last().unwrap().text;
    assert!(receipt.contains("paste clipped"), "{receipt}");
    assert!(
        receipt.contains(&format!(
            "{}/{} normalized bytes",
            control::MAX_COMPOSER_PASTE_BYTES,
            payload.len()
        )),
        "{receipt}"
    );
    assert!(receipt.contains("clipboard unchanged"), "{receipt}");
    assert!(receipt.contains("/mention <file>"), "{receipt}");
    assert!(app.thinking.is_none(), "clipped paste never submits");
}

#[test]
fn paste_cap_preserves_utf8_and_middle_caret_suffix() {
    let mut app = seed_preview_app();
    app.input = "αZ".to_string();
    app.cursor = 1;
    let payload = "🦀".repeat(control::MAX_COMPOSER_PASTE_BYTES / 4);

    app.on_paste(&payload);

    assert!(app.input.len() <= control::MAX_COMPOSER_PASTE_BYTES);
    assert!(app.input.starts_with('α'));
    assert!(app.input.ends_with('Z'));
    assert_eq!(
        app.cursor,
        1 + (app.input.chars().count() - 2),
        "caret lands after the admitted Unicode prefix"
    );
    assert!(app.messages.last().unwrap().text.contains("paste clipped"));
}

#[test]
fn paste_is_refused_once_the_draft_already_reaches_the_cap() {
    let mut app = seed_preview_app();
    app.input = "x".repeat(control::MAX_COMPOSER_PASTE_BYTES);
    app.cursor = app.input.chars().count();

    app.on_paste("🦀");

    assert_eq!(app.input.len(), control::MAX_COMPOSER_PASTE_BYTES);
    assert_eq!(app.cursor, control::MAX_COMPOSER_PASTE_BYTES);
    assert!(
        app.messages
            .last()
            .unwrap()
            .text
            .contains("inserted 0/4 normalized bytes")
    );
}

// ---------------------------------------------------------------------------
// Ctrl-V clipboard ingress
// ---------------------------------------------------------------------------

/// A real 2x2 PNG on disk for a fake clipboard tool to `cat`, so the bytes go
/// through the same admission `/see` uses. Removed on drop, panic included.
struct PngFixture {
    path: PathBuf,
}

impl PngFixture {
    fn new(label: &str) -> Self {
        Self::new_sized(label, 2)
    }

    fn new_sized(label: &str, size: u32) -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "angel-clipboard-{label}-{}-{}.png",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let mut bytes = Vec::new();
        image::DynamicImage::new_rgba8(size, size)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("encode the clipboard fixture");
        std::fs::write(&path, bytes).expect("write the clipboard fixture");
        Self { path }
    }

    /// `sh -c` argv that copies this PNG to stdout.
    fn read_argv(&self) -> Vec<String> {
        vec!["-c".to_string(), format!("cat {}", self.path.display())]
    }
}

impl Drop for PngFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn fail_argv() -> Vec<String> {
    vec!["-c".to_string(), "exit 1".to_string()]
}

fn shell(argv: &[&str]) -> Vec<String> {
    argv.iter().map(|arg| arg.to_string()).collect()
}

/// A clipboard offering `png` as its only image type (no text behind it).
fn screenshot_clipboard(png: &PngFixture) -> ClipboardPaste {
    ClipboardPaste::with_local("/bin/sh", vec![png.read_argv()], vec![fail_argv()])
}

/// A clipboard offering no image type and this text.
fn text_clipboard(text: &str) -> ClipboardPaste {
    let read = format!("printf '%s' '{text}'");
    ClipboardPaste::with_local(
        "/bin/sh",
        vec![fail_argv()],
        vec![shell(&["-c", read.as_str()])],
    )
}

/// A fake clipboard tool held open until the test opens its gate file, so a read
/// is genuinely in flight while the test keeps typing. The spin is bounded, so a
/// panicking test leaves no spinner behind.
struct GatedFixture {
    /// Held so the PNG fixture file outlives the script that `cat`s it.
    _png: PngFixture,
    gate: PathBuf,
    argv: Vec<String>,
}

impl GatedFixture {
    fn new(label: &str) -> Self {
        let png = PngFixture::new(label);
        let gate = std::env::temp_dir().join(format!(
            "angel-clipboard-gate-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&gate);
        let script = format!(
            "i=0; while [ ! -e {gate} ] && [ $i -lt 200 ]; do i=$((i+1)); sleep 0.02; done; \
             cat {png}",
            gate = gate.display(),
            png = png.path.display()
        );
        let argv = shell(&["-c", script.as_str()]);
        Self {
            _png: png,
            gate,
            argv,
        }
    }

    fn open(&self) {
        std::fs::write(&self.gate, b"go").expect("open the gate");
    }

    /// The clipboard the app reads while the gate is closed.
    fn clipboard(&self) -> ClipboardPaste {
        ClipboardPaste::with_local("/bin/sh", vec![self.argv.clone()], vec![fail_argv()])
    }
}

impl Drop for GatedFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.gate);
    }
}

fn ctrl_v() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL)
}

fn press(app: &mut crate::App, code: KeyCode) {
    app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn type_text(app: &mut crate::App, text: &str) {
    for character in text.chars() {
        press(app, KeyCode::Char(character));
    }
}

/// Poll until `probe` answers, then fail loudly instead of hanging.
fn wait_for<T>(what: &str, mut probe: impl FnMut() -> Option<T>) -> T {
    for _ in 0..600 {
        if let Some(value) = probe() {
            return value;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("{what} never happened");
}

/// Drive the real drain path (`App::advance`) until `done`.
fn advance_until(app: &mut crate::App, what: &str, mut done: impl FnMut(&crate::App) -> bool) {
    for _ in 0..600 {
        app.advance();
        if done(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("{what}: the clipboard read never landed");
}

fn stage_one(app: &mut crate::App, what: &str) {
    let before = app.clipboard_paste.staged_len();
    app.on_key(ctrl_v());
    assert!(
        app.clipboard_paste.loading(),
        "{what}: Ctrl-V must start a read"
    );
    advance_until(app, what, |app| app.clipboard_paste.staged_len() > before);
}

/// Operator turns appended after `baseline` (the seed history is not a turn).
fn sent_turns(app: &crate::App, baseline: usize) -> Vec<&ChatMsg> {
    app.history[baseline..]
        .iter()
        .filter(|message| message.role == ChatRole::User)
        .collect()
}

/// Attachments on the oldest accepted operator turn (ownership checks).
fn accepted_images(app: &crate::App, baseline: usize) -> usize {
    sent_turns(app, baseline)[0].attachments.len()
}

fn accepted_image_bytes(app: &crate::App, baseline: usize) -> u64 {
    crate::ui::clipboard::decoded_bytes(&sent_turns(app, baseline)[0].attachments[0])
}

/// The transcript receipt for the newest clipboard failure.
fn failure_receipt(app: &crate::App) -> String {
    app.messages
        .iter()
        .rev()
        .find(|message| message.text.contains("clipboard paste failed"))
        .expect("a clipboard failure is reported in the transcript")
        .text
        .to_string()
}

/// The composer caption for the staged images. Asserted without the leading
/// glyph: a terminal may spend two cells on it, and the count/bytes are the
/// contract.
fn expected_chip(app: &crate::App) -> String {
    let bytes = crate::ui::media::format_bytes(app.clipboard_paste.staged_bytes());
    match app.clipboard_paste.staged_len() {
        1 => format!("image {bytes}"),
        count => format!("{count} images {bytes}"),
    }
}

#[test]
fn ctrl_v_stages_a_screenshot_that_rides_the_next_turn() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let png = PngFixture::new("stage");
    app.clipboard_paste = screenshot_clipboard(&png);
    let baseline = app.history.len();

    app.on_key(ctrl_v());
    assert!(
        app.clipboard_paste.loading(),
        "Ctrl-V starts the read instead of blocking the UI"
    );
    advance_until(&mut app, "stage the screenshot", |app| {
        app.clipboard_paste.has_staged()
    });
    assert_eq!(app.clipboard_paste.staged_len(), 1);
    assert!(
        app.input.is_empty(),
        "staging never submits or writes a draft"
    );
    assert!(app.thinking.is_none(), "attaching an image is not a turn");

    // The attachment is visible on the composer chrome.
    let painted = render_app_text(&mut app, 120, 40);
    assert!(painted.contains(&expected_chip(&app)), "{painted}");
    assert!(painted.contains("Esc clears"), "{painted}");

    // A question typed after attaching keeps both.
    type_text(&mut app, "what is this?");
    app.submit();

    let turns = sent_turns(&app, baseline);
    assert_eq!(turns.len(), 1, "one operator turn was sent");
    assert_eq!(&*turns[0].content, "what is this?");
    assert_eq!(turns[0].attachments.len(), 1);
    assert!(matches!(turns[0].attachments[0], Media::Image { .. }));
    assert!(!app.clipboard_paste.has_staged(), "the send consumed it");
    assert!(app.input.is_empty(), "a real submit clears the composer");
}

#[test]
fn screenshot_with_an_empty_draft_sends_the_default_question() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let png = PngFixture::new("default-question");
    app.clipboard_paste = screenshot_clipboard(&png);
    let baseline = app.history.len();

    stage_one(&mut app, "stage the screenshot");
    assert!(app.input.trim().is_empty());

    press(&mut app, KeyCode::Enter);

    let turns = sent_turns(&app, baseline);
    assert_eq!(turns.len(), 1);
    assert_eq!(&*turns[0].content, crate::ui::clipboard::IMAGE_QUESTION);
    assert_eq!(turns[0].attachments.len(), 1);
    assert!(app.thinking.is_some(), "the image-only turn launched");
}

#[test]
fn multiple_screenshots_append_and_report_their_count() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let png = PngFixture::new("append");
    app.clipboard_paste = screenshot_clipboard(&png);
    let baseline = app.history.len();

    stage_one(&mut app, "stage the first screenshot");
    stage_one(&mut app, "stage the second screenshot");
    assert_eq!(
        app.clipboard_paste.staged_len(),
        2,
        "a second Ctrl-V appends"
    );

    let painted = render_app_text(&mut app, 120, 40);
    assert!(painted.contains(&expected_chip(&app)), "{painted}");
    assert!(painted.contains("2 images"), "{painted}");

    type_text(&mut app, "compare these");
    app.submit();

    let turns = sent_turns(&app, baseline);
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].attachments.len(), 2);
    assert!(!app.clipboard_paste.has_staged(), "the send consumed both");
}

#[test]
fn enter_during_the_read_is_honored_with_the_image() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let gated = GatedFixture::new("deferred");
    app.clipboard_paste = gated.clipboard();
    let baseline = app.history.len();

    app.on_key(ctrl_v());
    assert!(app.clipboard_paste.loading(), "the fake tool is held open");
    type_text(&mut app, "read this");
    press(&mut app, KeyCode::Enter);
    assert!(
        sent_turns(&app, baseline).is_empty(),
        "the send waits for the bytes instead of going out text-only"
    );
    assert_eq!(app.input, "read this", "the draft survives the wait");

    gated.open();
    advance_until(&mut app, "deferred send", |app| app.thinking.is_some());

    let turns = sent_turns(&app, baseline);
    assert_eq!(turns.len(), 1);
    assert_eq!(&*turns[0].content, "read this");
    assert_eq!(turns[0].attachments.len(), 1);
}

#[test]
fn a_second_ctrl_v_while_reading_does_not_start_a_second_read() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let gated = GatedFixture::new("single-flight");
    app.clipboard_paste = gated.clipboard();

    app.on_key(ctrl_v());
    app.on_key(ctrl_v());
    gated.open();
    advance_until(&mut app, "stage the screenshot", |app| {
        app.clipboard_paste.has_staged()
    });

    // Two reads of the same clipboard would stage two images.
    assert_eq!(app.clipboard_paste.staged_len(), 1);
}

#[test]
fn a_missing_clipboard_tool_is_reported_and_recoverable() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.clipboard_paste = ClipboardPaste::with_local(
        "/nonexistent/angel-clipboard-tool",
        vec![fail_argv()],
        vec![fail_argv()],
    );
    let baseline = app.history.len();

    app.on_key(ctrl_v());
    advance_until(&mut app, "report the missing tool", |app| {
        !app.clipboard_paste.loading()
    });

    let receipt = failure_receipt(&app);
    assert!(receipt.contains("not installed"), "{receipt}");
    assert!(!app.clipboard_paste.has_staged());

    // Recovery is ordinary use: the operator's text still sends, alone, and a
    // later Ctrl-V against a working tool stages normally (no auto-send).
    type_text(&mut app, "no image after all");
    app.submit();
    let turns = sent_turns(&app, baseline);
    assert_eq!(turns.len(), 1);
    assert!(turns[0].attachments.is_empty());

    let png = PngFixture::new("recovered");
    app.clipboard_paste = screenshot_clipboard(&png);
    stage_one(&mut app, "stage after recovery");
    assert_eq!(app.clipboard_paste.staged_len(), 1);
}

#[test]
fn a_session_without_a_display_reports_no_backend() {
    let _guard = env_lock();
    let _wayland = TestEnvGuard::unset("WAYLAND_DISPLAY");
    let _display = TestEnvGuard::unset("DISPLAY");
    let mut app = seed_preview_app();

    app.on_key(ctrl_v());
    advance_until(&mut app, "report the missing backend", |app| {
        !app.clipboard_paste.loading()
    });

    let receipt = failure_receipt(&app);
    assert!(receipt.contains("no clipboard backend"), "{receipt}");
    assert!(receipt.contains("WAYLAND_DISPLAY"), "{receipt}");
}

#[test]
fn malformed_clipboard_bytes_are_rejected_by_the_shared_admission_policy() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let html = "printf '<html>not an image</html>'";
    app.clipboard_paste =
        ClipboardPaste::with_local("/bin/sh", vec![shell(&["-c", html])], vec![fail_argv()]);

    app.on_key(ctrl_v());
    advance_until(&mut app, "report the malformed image", |app| {
        !app.clipboard_paste.loading()
    });

    let receipt = failure_receipt(&app);
    assert!(receipt.contains("clipboard image"), "{receipt}");
    assert!(
        receipt.contains("unsupported image format"),
        "the /see admission policy owns this rejection: {receipt}"
    );
    assert!(!app.clipboard_paste.has_staged());
    assert!(app.thinking.is_none());
}

#[test]
fn an_oversized_clipboard_image_is_reported_without_a_text_fallback() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let oversized = format!(
        "head -c {} /dev/zero",
        crate::agent::club::MAX_IMAGE_ATTACHMENT_BYTES + 1
    );
    let text = "printf 'text must not be pasted'";
    app.clipboard_paste = ClipboardPaste::with_local(
        "/bin/sh",
        vec![shell(&["-c", oversized.as_str()])],
        vec![shell(&["-c", text])],
    );

    app.on_key(ctrl_v());
    advance_until(&mut app, "report the oversize", |app| {
        !app.clipboard_paste.loading()
    });

    let receipt = failure_receipt(&app);
    assert!(receipt.contains("exceeds the 5 MiB"), "{receipt}");
    assert!(!app.clipboard_paste.has_staged());
    assert!(
        app.input.is_empty(),
        "an oversized image must not fall back to reading text"
    );
}

#[test]
fn a_wedged_clipboard_tool_is_killed_and_reaped() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.clipboard_paste = ClipboardPaste::with_local(
        "/bin/sh",
        vec![shell(&["-c", "exec sleep 30"])],
        vec![fail_argv()],
    );
    app.clipboard_paste
        .set_deadline(std::time::Duration::from_millis(200));

    app.on_key(ctrl_v());
    let pid = wait_for("the wedged tool started", || {
        app.clipboard_paste.child_pid()
    });
    advance_until(&mut app, "report the wedged tool", |app| {
        app.messages
            .iter()
            .any(|message| message.text.contains("did not answer within"))
    });

    let receipt = failure_receipt(&app);
    assert!(receipt.contains("did not answer within"), "{receipt}");
    assert!(
        app.clipboard_paste.child_pid().is_none(),
        "the reader reaped its own child (pid {pid})"
    );
    #[cfg(target_os = "linux")]
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "a killed tool must be reaped, not left as a zombie"
    );
    assert!(!app.clipboard_paste.loading(), "Ctrl-V is usable again");

    // The next read is a fresh, bounded read rather than an orphaned retry.
    app.on_key(ctrl_v());
    assert!(app.clipboard_paste.loading());
    advance_until(&mut app, "settle the retry", |app| {
        !app.clipboard_paste.loading()
    });
}

#[test]
fn text_clipboard_falls_back_to_an_ordinary_paste() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.clipboard_paste = text_clipboard("line one");
    let baseline = app.history.len();

    app.on_key(ctrl_v());
    advance_until(&mut app, "paste the text", |app| {
        app.input.ends_with("line one")
    });

    assert_eq!(app.input, "line one");
    assert!(!app.clipboard_paste.has_staged());
    assert!(app.thinking.is_none(), "a text paste never submits");
    assert!(sent_turns(&app, baseline).is_empty());
}

#[test]
fn image_only_clipboard_reports_that_it_holds_no_text() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    app.clipboard_paste =
        ClipboardPaste::with_local("/bin/sh", vec![fail_argv()], vec![fail_argv()]);

    app.on_key(ctrl_v());
    advance_until(&mut app, "report the empty clipboard", |app| {
        !app.clipboard_paste.loading()
    });

    let receipt = failure_receipt(&app);
    assert!(receipt.contains("no image and no text"), "{receipt}");
    assert!(app.input.is_empty());
}

#[test]
fn staged_screenshot_survives_local_commands_and_esc_removes_it() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let png = PngFixture::new("esc");
    app.clipboard_paste = screenshot_clipboard(&png);

    stage_one(&mut app, "stage the screenshot");

    // A display-only command is not the screenshot's send.
    type_text(&mut app, "/status");
    app.submit();
    assert!(
        app.clipboard_paste.has_staged(),
        "a local command must not consume the attachment"
    );

    // Esc removes the image while keeping the typed draft.
    type_text(&mut app, "keep my words");
    press(&mut app, KeyCode::Esc);
    assert!(!app.clipboard_paste.has_staged());
    assert_eq!(app.input, "keep my words", "removal never clears the draft");
    assert!(
        app.messages
            .last()
            .unwrap()
            .text
            .contains("staged screenshot removed")
    );
    let painted = render_app_text(&mut app, 120, 40);
    assert!(!painted.contains("Esc clears"), "{painted}");
}

#[test]
fn esc_keeps_interrupting_a_running_turn_instead_of_removing_images() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let png = PngFixture::new("busy-esc");
    app.clipboard_paste = screenshot_clipboard(&png);
    stage_one(&mut app, "stage the screenshot");

    app.thinking = Some(crate::agent::turn::Thinking::pending_for_test("practice"));
    press(&mut app, KeyCode::Esc);

    assert!(
        app.clipboard_paste.has_staged(),
        "a running turn owns Esc; the staged image stays for the next message"
    );
    assert!(
        app.thinking
            .as_ref()
            .is_some_and(|t| t.cancel.load(Ordering::Relaxed)),
        "Esc still interrupted the turn"
    );
}

#[test]
fn an_open_modal_or_focused_shell_keeps_ctrl_v_for_itself() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let png = PngFixture::new("ownership");
    app.clipboard_paste = screenshot_clipboard(&png);
    app.input = "draft".to_string();
    app.cursor = app.input.chars().count();

    app.shell_focused = true;
    app.on_key(ctrl_v());
    assert!(!app.clipboard_paste.loading(), "the shell pane owns Ctrl-V");

    app.shell_focused = false;
    app.bag = crate::agent::club::Bag::for_reasoning_render_test();
    app.open_agent_menu(crate::ui::agent_panel::controls::AgentMenuKind::Model);
    assert!(
        app.agent_menu.is_some(),
        "the route menu opened for this bag"
    );
    app.on_key(ctrl_v());
    assert!(
        !app.clipboard_paste.loading(),
        "an open agent menu owns Ctrl-V until it is dismissed"
    );

    // Ctrl-V still belongs to the composer once nothing else owns input.
    app.agent_menu = None;
    app.on_key(ctrl_v());
    advance_until(&mut app, "stage the screenshot", |app| {
        app.clipboard_paste.has_staged()
    });
    assert_eq!(app.clipboard_paste.staged_len(), 1);
    assert_eq!(app.input, "draft", "Ctrl-V never rewrote the draft");
}

#[test]
fn an_image_paste_steers_while_a_turn_is_running() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let png = PngFixture::new("steer");
    app.clipboard_paste = screenshot_clipboard(&png);
    stage_one(&mut app, "stage the screenshot");

    app.thinking = Some(crate::agent::turn::Thinking::pending_for_test("practice"));
    type_text(&mut app, "look at this while you work");
    press(&mut app, KeyCode::Enter);

    assert_eq!(app.steer_queue.len(), 1, "the message steered");
    let steers = app.steer_queue.drain();
    assert_eq!(&*steers[0].content, "look at this while you work");
    assert_eq!(steers[0].attachments.len(), 1);
    assert!(
        !app.clipboard_paste.has_staged(),
        "the steer took the image"
    );
    assert!(app.input.is_empty());
}

#[test]
fn an_image_only_message_steers_with_the_default_question() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let png = PngFixture::new("steer-only");
    app.clipboard_paste = screenshot_clipboard(&png);
    stage_one(&mut app, "stage the screenshot");

    app.thinking = Some(crate::agent::turn::Thinking::pending_for_test("practice"));
    press(&mut app, KeyCode::Enter);

    let steers = app.steer_queue.drain();
    assert_eq!(steers.len(), 1);
    assert_eq!(&*steers[0].content, crate::ui::clipboard::IMAGE_QUESTION);
    assert_eq!(steers[0].attachments.len(), 1);
}

#[test]
fn a_later_paste_never_retrofits_an_accepted_turn() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    // Different raster sizes, so the two screenshots are distinguishable by
    // their attachment payload and not only by count.
    let first = PngFixture::new_sized("first-image", 2);
    let second = PngFixture::new_sized("second-image", 3);
    app.clipboard_paste = screenshot_clipboard(&first);
    let baseline = app.history.len();

    stage_one(&mut app, "stage the first screenshot");
    let first_bytes = app.clipboard_paste.staged_bytes();
    type_text(&mut app, "first");
    app.submit();
    assert!(app.thinking.is_some(), "the turn is running");
    assert_eq!(accepted_images(&app, baseline), 1);
    assert_eq!(accepted_image_bytes(&app, baseline), first_bytes);

    // A new paste while the turn runs belongs to the *next* message.
    app.clipboard_paste = screenshot_clipboard(&second);
    stage_one(&mut app, "stage the second screenshot");
    let second_bytes = app.clipboard_paste.staged_bytes();
    assert_ne!(first_bytes, second_bytes, "the fixtures differ");
    assert_eq!(
        accepted_images(&app, baseline),
        1,
        "a later paste must not retrofit the accepted turn"
    );
    assert_eq!(accepted_image_bytes(&app, baseline), first_bytes);

    press(&mut app, KeyCode::Enter);
    let steers = app.steer_queue.drain();
    assert_eq!(steers.len(), 1, "the new image steered as its own message");
    assert_eq!(steers[0].attachments.len(), 1);
    assert_eq!(
        crate::ui::clipboard::decoded_bytes(&steers[0].attachments[0]),
        second_bytes,
        "the steer carries the new screenshot"
    );
    assert_eq!(accepted_images(&app, baseline), 1);
    assert_eq!(
        accepted_image_bytes(&app, baseline),
        first_bytes,
        "the accepted turn still holds exactly its own screenshot"
    );
}

#[test]
fn a_canceled_parked_turn_restores_its_images_and_draft() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let png = PngFixture::new("cancel");
    app.clipboard_paste = screenshot_clipboard(&png);
    app.submit_deferral = true; // park the turn so nothing has launched

    stage_one(&mut app, "stage the screenshot");
    type_text(&mut app, "hold this");
    press(&mut app, KeyCode::Enter);
    assert!(
        app.pending_turn.is_some(),
        "the turn is parked for its echo"
    );
    assert!(
        !app.clipboard_paste.has_staged(),
        "the parked turn owns the image until it is canceled"
    );

    press(&mut app, KeyCode::Esc);

    assert!(app.pending_turn.is_none(), "Esc canceled the parked turn");
    assert_eq!(app.input, "hold this", "the draft came back");
    assert_eq!(
        app.clipboard_paste.staged_len(),
        1,
        "the canceled turn's image came back too"
    );
}

#[test]
fn graceful_exit_retains_the_parked_turns_image() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let png = PngFixture::new("exit");
    app.clipboard_paste = screenshot_clipboard(&png);
    app.submit_deferral = true;

    stage_one(&mut app, "stage the screenshot");
    type_text(&mut app, "keep this");
    press(&mut app, KeyCode::Enter);
    assert!(app.pending_turn.is_some());

    type_text(&mut app, "exit");
    press(&mut app, KeyCode::Enter);

    let retained = app
        .history
        .iter()
        .rev()
        .find(|message| &*message.content == "keep this")
        .expect("the accepted-but-unlaunched turn reached history at exit");
    assert_eq!(retained.attachments.len(), 1);
    assert!(app.should_quit, "the graceful exit completed");
}

#[test]
fn a_new_chat_drops_staged_screenshots() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let png = PngFixture::new("new-chat");
    app.clipboard_paste = screenshot_clipboard(&png);
    stage_one(&mut app, "stage the screenshot");

    type_text(&mut app, "/new");
    app.submit();

    assert!(
        !app.clipboard_paste.has_staged(),
        "a fresh session must not inherit the old thread's screenshot"
    );
}

#[test]
fn a_project_boundary_drops_staged_screenshots() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let png = PngFixture::new("project-boundary");
    app.clipboard_paste = screenshot_clipboard(&png);
    stage_one(&mut app, "stage the screenshot");

    let workspace = crate::tests::TestGitWorkspace::new("clipboard-boundary");
    app.change_workspace(workspace.path().to_str());

    assert!(
        !app.clipboard_paste.has_staged(),
        "a project boundary must not leak a screenshot into the next project"
    );
}

/// Wait for a canceled read's tool to be reaped.
fn assert_reaped(pid: u32, what: &str) {
    for _ in 0..600 {
        if !crate::ui::clipboard::process_is_running(pid) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("{what}: pid {pid} is still running");
}

#[test]
fn clearing_cancels_an_in_flight_read() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    // The fake tool is held open by its gate, so only a real cancel ends it.
    let gated = GatedFixture::new("clear-cancel");
    app.clipboard_paste = gated.clipboard();

    app.on_key(ctrl_v());
    let pid = wait_for("the fake tool started", || app.clipboard_paste.child_pid());
    assert!(app.clipboard_paste.loading());

    type_text(&mut app, "/new");
    app.submit();

    assert!(!app.clipboard_paste.loading(), "the read was forgotten");
    assert!(
        !app.clipboard_paste.take_deferred_submit(),
        "a cleared read cannot fire a deferred send"
    );
    assert_reaped(pid, "a cleared read ends its tool");
    for _ in 0..10 {
        app.advance();
    }
    assert!(
        !app.clipboard_paste.has_staged(),
        "nothing from the superseded read reaches the new session"
    );
}

#[test]
fn dropping_the_paste_cancels_an_in_flight_read() {
    let _guard = env_lock();
    let gated = GatedFixture::new("drop-cancel");
    let mut paste = gated.clipboard();

    paste.start();
    let pid = wait_for("the fake tool started", || paste.child_pid());
    drop(paste);

    assert_reaped(pid, "a dropped paste ends its tool");
}

#[test]
fn cancel_restores_only_the_pasted_screenshot_not_the_see_attachment() {
    let _guard = env_lock();
    let mut app = seed_preview_app();
    let see_png = PngFixture::new("see-attachment");
    let pasted = PngFixture::new("pasted-screenshot");
    app.clipboard_paste = screenshot_clipboard(&pasted);
    app.submit_deferral = true;

    stage_one(&mut app, "stage the screenshot");
    let command = format!("/see {} describe it", see_png.path.display());
    app.input = command.clone();
    app.cursor = command.chars().count();
    press(&mut app, KeyCode::Enter);

    let parked = app.pending_turn.as_ref().expect("the turn parked");
    assert_eq!(parked.user_msg.attachments.len(), 2, "see + pasted");
    assert_eq!(
        parked.clipboard_images, 1,
        "only the pasted screenshot came from the composer"
    );

    press(&mut app, KeyCode::Esc);

    assert!(app.pending_turn.is_none());
    assert_eq!(app.input, command, "the /see command came back");
    assert_eq!(
        app.clipboard_paste.staged_len(),
        1,
        "only the pasted screenshot is staged again; /see is rebuilt from the draft"
    );

    // Resubmitting reconstructs the /see image without duplicating it.
    press(&mut app, KeyCode::Enter);
    let parked = app.pending_turn.as_ref().expect("the turn parked again");
    assert_eq!(
        parked.user_msg.attachments.len(),
        2,
        "see + pasted, never see twice"
    );
    assert_eq!(parked.clipboard_images, 1);
}
