//! Operator clipboard → composer image ingress for Ctrl-V.
//!
//! The cockpit *writes* to the clipboard itself (OSC-52, see
//! `deliver_to_clipboard`), but there is no portable OSC-52 *read*, so a
//! screenshot comes from the session's native clipboard tool: `wl-paste` under
//! Wayland, `xclip` under X11. The bytes go through the same
//! [`Media::image_from_bytes`] admission `/see` uses — nothing is written to the
//! repository or to a temporary file.
//!
//! Three properties are deliberate here:
//!
//! - **Typed reads.** Every attempt names the MIME family it wants
//!   (`--type image/png`, `--type text`, …). Tool-side inference must never hand
//!   a binary image stream to the text fallback, and a text paste must never
//!   arrive as raw raster bytes.
//! - **One owned reader thread.** A read runs the tool in its own process group
//!   and drains stdout without blocking: non-blocking pipe reads, `try_wait`, and
//!   a bounded poll loop that respects an absolute deadline and a cancel flag. A
//!   tool that closes stdout and stays alive, or leaves a descendant holding the
//!   pipe, cannot wedge the read.
//! - **No leaked work.** Every exit path ends the tool's process *group* and
//!   reaps the direct child, so a stuck clipboard never leaves a worker, a
//!   descendant, or a permanently unavailable Ctrl-V behind.

use crate::agent::club::{ChatMsg, MAX_IMAGE_ATTACHMENT_BYTES, Media};
use crate::agent::sandbox::process_owner::{Child, OwnedCommandExt};
use std::io::Read as _;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt as _;
use std::process::{ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

/// Default question when a screenshot is sent with an empty draft — the same
/// wording `/see` uses for a bare image, so the two paths behave alike.
pub(crate) const IMAGE_QUESTION: &str = "Describe this image.";

/// How long one clipboard read may take before it is ended and reported as
/// failed. Clipboard tools answer in milliseconds; a wedged clipboard owner must
/// not take Ctrl-V away for the rest of the session.
const FETCH_DEADLINE: Duration = Duration::from_secs(5);

/// How long the reader waits for output before re-checking cancellation and the
/// deadline. Small enough that a canceled or overdue read ends promptly.
const READ_POLL: Duration = Duration::from_millis(25);

/// One read's control block, shared with the paste state so Esc, `/new`, a
/// project boundary, or a dropped paste can end a read that is still running.
#[derive(Default)]
struct ReadControl {
    /// Set by the UI thread when the read is no longer wanted.
    cancel: AtomicBool,
    /// Direct-child PID while the tool runs (0 when none). Test observation only:
    /// the reader owns the child and is the only killer and reaper.
    pid: AtomicU32,
}

/// One bounded tool run's outcome.
#[derive(Debug)]
enum ToolRead {
    Bytes(Vec<u8>),
    /// The clipboard offered this kind beyond the caller's limit. Reported
    /// before the ended child's status can masquerade as a tool failure.
    TooLarge,
    /// The tool ran and offered nothing of the requested type.
    Empty(String),
    /// The read failed: launch/reap error, unreadable stdout, deadline, or cancel.
    Failed(String),
}

/// What the clipboard held. Text keeps Ctrl-V working as an ordinary paste.
#[derive(Debug)]
enum Fetched {
    Image(Media),
    Text(String),
}

/// Outcome of one drained read.
#[derive(Debug)]
pub(crate) enum Drained {
    /// Admitted image(s) are staged for the next submitted turn.
    Staged,
    /// No image was offered; the text pastes like a bracketed paste.
    Text(String),
    /// The read failed. The draft is untouched and the operator sees why.
    Failed(String),
}

/// Where one read gets its bytes.
enum Plan {
    /// The session clipboard tool selected from this session's environment.
    Session,
    /// Local fake commands: the same bounded reader and the same classifier, so
    /// suites can drive oversize, closed-stdout, descendant, timeout and
    /// cancellation behavior without touching the desktop clipboard.
    #[cfg(test)]
    Local {
        program: String,
        image: Vec<Vec<String>>,
        text: Vec<Vec<String>>,
    },
}

struct Fetch {
    rx: Receiver<Result<Fetched, String>>,
    /// Cancellation for this read (shared with its reader thread).
    control: Arc<ReadControl>,
}

/// Composer-owned clipboard paste state: at most one read in flight plus any
/// number of staged screenshots, all display-only until a real submit consumes
/// them.
pub(crate) struct ClipboardPaste {
    plan: Arc<Plan>,
    /// Read deadline; production 5 s, tests shrink it.
    deadline: Duration,
    fetch: Option<Fetch>,
    staged: Vec<Media>,
    /// Enter pressed while a read was in flight; one flag, one source of truth.
    deferred_submit: bool,
}

impl ClipboardPaste {
    pub(crate) fn new() -> Self {
        Self::fresh(Plan::Session)
    }

    /// A paste that reads the clipboard through `plan`, with no read or staged
    /// image yet.
    fn fresh(plan: Plan) -> Self {
        Self {
            plan: Arc::new(plan),
            deadline: FETCH_DEADLINE,
            fetch: None,
            staged: Vec::new(),
            deferred_submit: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_local(
        program: &str,
        image: Vec<Vec<String>>,
        text: Vec<Vec<String>>,
    ) -> Self {
        Self::fresh(Plan::Local {
            program: program.to_string(),
            image,
            text,
        })
    }

    #[cfg(test)]
    pub(crate) fn set_deadline(&mut self, deadline: Duration) {
        self.deadline = deadline;
    }

    /// Start reading the clipboard. Non-blocking; [`Self::poll`] collects it.
    /// A read already in flight is left alone (its answer is still the newest).
    pub(crate) fn start(&mut self) {
        if self.fetch.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let plan = Arc::clone(&self.plan);
        let control = Arc::new(ReadControl::default());
        let reader_control = Arc::clone(&control);
        let deadline = Instant::now() + self.deadline;
        let deadline_secs = self.deadline.as_secs();
        let spawned = std::thread::Builder::new()
            .name("clipboard-paste".into())
            .spawn(move || {
                let outcome = fetch_with(&reader_control, deadline, deadline_secs, &plan);
                let _ = tx.send(outcome);
            });
        match spawned {
            Ok(_) => self.fetch = Some(Fetch { rx, control }),
            Err(error) => {
                let (tx, rx) = mpsc::channel();
                let _ = tx.send(Err(format!(
                    "could not start the clipboard reader: {error} — press Ctrl-V to retry"
                )));
                self.fetch = Some(Fetch {
                    rx,
                    control: Arc::new(ReadControl::default()),
                });
            }
        }
    }

    /// True while a read is still running (the composer keeps the draft).
    pub(crate) fn loading(&self) -> bool {
        self.fetch.is_some()
    }

    pub(crate) fn has_staged(&self) -> bool {
        !self.staged.is_empty()
    }

    pub(crate) fn staged_len(&self) -> usize {
        self.staged.len()
    }

    #[cfg(test)]
    pub(crate) fn child_pid(&self) -> Option<u32> {
        let pid = self.fetch.as_ref()?.control.pid.load(Ordering::Acquire);
        (pid != 0).then_some(pid)
    }

    /// Accept an Enter that arrived mid-read (see the field docs).
    pub(crate) fn defer_submit(&mut self) {
        self.deferred_submit = true;
    }

    /// Consume one deferred Enter. False when the last read produced no payload:
    /// a failed read must not send the draft without its intended image.
    pub(crate) fn take_deferred_submit(&mut self) -> bool {
        std::mem::take(&mut self.deferred_submit)
    }

    /// Drop every staged screenshot and forget any read or deferred send still in
    /// flight (operator removal, `/new`, and a project boundary). Forgetting the
    /// read is what stops a superseded Ctrl-V from delivering a screenshot into
    /// the next session.
    pub(crate) fn clear(&mut self) {
        self.cancel_read();
        self.staged.clear();
        self.deferred_submit = false;
    }

    /// End an in-flight read. The reader thread kills its tool group and reaps
    /// the direct child; the dropped receiver swallows its answer.
    fn cancel_read(&mut self) {
        if let Some(fetch) = self.fetch.take() {
            fetch.control.cancel.store(true, Ordering::Release);
        }
    }

    /// Fold the staged screenshots into a message the operator is actually
    /// sending, and report how many were appended: a canceled turn restores
    /// exactly those, never the `/see` attachments it also carries.
    pub(crate) fn attach_to(&mut self, message: &mut ChatMsg) -> usize {
        if self.staged.is_empty() {
            return 0;
        }
        let mut parts = message.attachments.to_vec();
        let count = self.staged.len();
        parts.append(&mut self.staged);
        message.attachments = parts.into();
        count
    }

    /// Put back the screenshots a canceled turn took from the composer. They are
    /// the last `count` attachments of the unsent message; anything earlier came
    /// from `/see` and its draft text reconstructs it on resubmit.
    pub(crate) fn restore_images(&mut self, attachments: &[Media], count: usize) {
        let start = attachments.len().saturating_sub(count);
        self.staged.extend(
            attachments[start..]
                .iter()
                .filter(|media| matches!(media, Media::Image { .. }))
                .cloned(),
        );
    }

    /// Composer title chip: how many images are staged and how big they are.
    /// The caption and the count are the contract (the leading glyph is
    /// decoration, and a terminal may spend two cells on it).
    pub(crate) fn chip(&self, can_remove: bool) -> Option<String> {
        let bytes = crate::ui::media::format_bytes(self.staged_bytes());
        let label = match self.staged.len() {
            0 => return None,
            1 => format!("📎 image {bytes}"),
            count => format!("📎 {count} images {bytes}"),
        };
        let hint = if can_remove { " · Esc clears" } else { "" };
        Some(format!(" {label}{hint} "))
    }

    /// Total decoded bytes of the staged images.
    pub(crate) fn staged_bytes(&self) -> u64 {
        self.staged.iter().map(decoded_bytes).sum()
    }

    /// Collect a finished read. Cheap when nothing is in flight; the reader's own
    /// deadline bounds how long an in-flight read can stay unanswered.
    pub(crate) fn poll(&mut self) -> Option<Drained> {
        let fetch = self.fetch.as_ref()?;
        let outcome = match fetch.rx.try_recv() {
            Ok(outcome) => outcome,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("the clipboard reader stopped before answering — press Ctrl-V to retry".into())
            }
        };
        self.fetch = None;
        match outcome {
            Ok(Fetched::Image(media)) => {
                self.staged.push(media);
                Some(Drained::Staged)
            }
            // A deferred Enter survives a successful read and is consumed by the
            // caller; the flag has exactly one writer.
            Ok(Fetched::Text(text)) => Some(Drained::Text(text)),
            Err(error) => {
                // A failed read must not fire the text-only send it deferred.
                self.deferred_submit = false;
                Some(Drained::Failed(error))
            }
        }
    }
}

impl Drop for ClipboardPaste {
    fn drop(&mut self) {
        // A read must not outlive the composer that asked for it: the reader ends
        // its tool group and reaps the child on the next poll.
        self.cancel_read();
    }
}

/// Decoded size of an attachment without re-encoding it (base64 is 4 chars per
/// 3 bytes plus padding). The chip measures with this, and ownership checks use
/// it to tell two staged screenshots apart.
pub(crate) fn decoded_bytes(media: &Media) -> u64 {
    let Media::Image { b64, .. } = media else {
        return 0;
    };
    let padding = b64.bytes().rev().take_while(|byte| *byte == b'=').count() as u64;
    (b64.len() as u64 / 4)
        .saturating_mul(3)
        .saturating_sub(padding)
}

/// One clipboard backend: the session clipboard is owned by the compositor or
/// the X server, so the CLI is the only reader the cockpit has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    Wayland,
    X11,
}

impl Backend {
    fn program(self) -> &'static str {
        match self {
            Self::Wayland => "wl-paste",
            Self::X11 => "xclip",
        }
    }

    /// Typed image attempts, most specific first. The generic `image` name makes
    /// `wl-paste` pick any offered image type, which admission then accepts or
    /// rejects explicitly.
    fn image_args(self) -> &'static [&'static [&'static str]] {
        match self {
            Self::Wayland => &[
                &["--type", "image/png"],
                &["--type", "image/jpeg"],
                &["--type", "image"],
            ],
            Self::X11 => &[
                &["-selection", "clipboard", "-t", "image/png", "-o"],
                &["-selection", "clipboard", "-t", "image/jpeg", "-o"],
            ],
        }
    }

    /// Typed text attempts. Never inference: an image-only clipboard must yield
    /// "no text", not a raster byte stream.
    fn text_args(self) -> &'static [&'static [&'static str]] {
        match self {
            Self::Wayland => &[
                &["--type", "text/plain", "--no-newline"],
                &["--type", "text", "--no-newline"],
            ],
            Self::X11 => &[
                &["-selection", "clipboard", "-t", "UTF8_STRING", "-o"],
                &["-selection", "clipboard", "-t", "text/plain", "-o"],
            ],
        }
    }
}

/// What one kind of read produced.
enum KindRead {
    Bytes(Vec<u8>),
    /// The clipboard offered this kind beyond the limit: the caller reports it
    /// instead of trying another kind.
    Over,
    /// The read could not be launched: a hard failure, not "no such type".
    Failed(String),
    /// Nothing of this kind is on the clipboard.
    None(String),
}

fn fetch_with(
    control: &ReadControl,
    deadline: Instant,
    deadline_secs: u64,
    plan: &Plan,
) -> Result<Fetched, String> {
    match plan {
        Plan::Session => {
            let backend = backend()?;
            // Wayland is preferred because that is the session the cockpit draws
            // in and XWayland's copy of a compositor selection can lag the real one.
            let image = borrowed(backend.image_args());
            let text = borrowed(backend.text_args());
            fetch_kinds(
                control,
                deadline,
                deadline_secs,
                backend.program(),
                &image,
                &text,
            )
        }
        #[cfg(test)]
        Plan::Local {
            program,
            image,
            text,
        } => {
            let image: Vec<Vec<&str>> = image
                .iter()
                .map(|args| args.iter().map(String::as_str).collect())
                .collect();
            let text: Vec<Vec<&str>> = text
                .iter()
                .map(|args| args.iter().map(String::as_str).collect())
                .collect();
            fetch_kinds(control, deadline, deadline_secs, program, &image, &text)
        }
    }
}

fn borrowed<'a>(table: &'a [&'a [&'a str]]) -> Vec<Vec<&'a str>> {
    table.iter().map(|args| args.to_vec()).collect()
}

/// Read the clipboard once: image bytes when offered, otherwise text.
fn fetch_kinds(
    control: &ReadControl,
    deadline: Instant,
    deadline_secs: u64,
    program: &str,
    image: &[Vec<&str>],
    text: &[Vec<&str>],
) -> Result<Fetched, String> {
    let image = read_kind(
        control,
        deadline,
        deadline_secs,
        program,
        image,
        MAX_IMAGE_ATTACHMENT_BYTES,
    );
    match image {
        KindRead::Bytes(bytes) => {
            return Media::image_from_bytes(&bytes, "clipboard image").map(Fetched::Image);
        }
        KindRead::Over => {
            return Err(format!(
                "clipboard image exceeds the {} MiB attachment limit",
                MAX_IMAGE_ATTACHMENT_BYTES / (1024 * 1024)
            ));
        }
        KindRead::Failed(detail) => return Err(detail),
        KindRead::None(_) => {}
    }
    match read_kind(
        control,
        deadline,
        deadline_secs,
        program,
        text,
        crate::app::control::MAX_COMPOSER_PASTE_BYTES,
    ) {
        KindRead::Bytes(bytes) => Ok(Fetched::Text(String::from_utf8_lossy(&bytes).into_owned())),
        KindRead::Over => Err(format!(
            "clipboard text is larger than the {} KiB composer paste cap — clipboard unchanged; \
             save it to a file and use /mention <file>",
            crate::app::control::MAX_COMPOSER_PASTE_BYTES / 1024
        )),
        KindRead::Failed(detail) => Err(detail),
        KindRead::None(detail) => Err(if detail.is_empty() {
            format!("{program} found no image and no text on the clipboard")
        } else {
            format!("{detail} · the clipboard holds no image and no text")
        }),
    }
}

/// Try each typed candidate of one kind until one answers with bytes.
fn read_kind(
    control: &ReadControl,
    deadline: Instant,
    deadline_secs: u64,
    program: &str,
    candidates: &[Vec<&str>],
    limit: usize,
) -> KindRead {
    let mut last = String::new();
    for args in candidates {
        if control.cancel.load(Ordering::Acquire) {
            return KindRead::Failed("the clipboard read was canceled".to_string());
        }
        match read_tool(control, deadline, deadline_secs, program, args, limit) {
            ToolRead::Bytes(bytes) if !bytes.is_empty() => return KindRead::Bytes(bytes),
            ToolRead::Bytes(_) => {}
            ToolRead::TooLarge => return KindRead::Over,
            ToolRead::Empty(detail) => last = detail,
            ToolRead::Failed(detail) => return KindRead::Failed(detail),
        }
    }
    KindRead::None(last)
}

/// Why a read stopped.
enum Stop {
    /// The tool exited; whatever it wrote is its answer.
    Exited,
    /// The tool closed stdout while it was still alive: the answer is complete,
    /// and waiting for an exit would hang on a tool that never takes one.
    Eof,
    /// One byte past the admission limit.
    Over,
    Deadline,
    Canceled,
    Failed(String),
}

/// Run one typed clipboard tool and read its stdout without blocking.
///
/// The tool leads its own process group, stdout is non-blocking, and the loop
/// polls cancellation, the absolute deadline, and the child's exit. Every return
/// path ends the tool's process group (a descendant holding the pipe cannot keep
/// the read alive) and reaps the direct child.
fn read_tool(
    control: &ReadControl,
    deadline: Instant,
    deadline_secs: u64,
    program: &str,
    args: &[&str],
    limit: usize,
) -> ToolRead {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // Its own process group: one group kill ends the whole tree the tool built.
    command.process_group(0);
    let mut child = match command.spawn_owned() {
        Ok(child) => child,
        Err(error) => return ToolRead::Failed(spawn_error(program, error)),
    };
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return ToolRead::Failed(format!("{program} offered no output pipe"));
    };
    let pid = child.id();
    control.pid.store(pid, Ordering::Release);
    if let Err(error) = set_nonblocking(stdout.as_raw_fd()) {
        end_tool(&mut child, pid);
        control.pid.store(0, Ordering::Release);
        return ToolRead::Failed(format!(
            "{program} stdout could not be read without blocking: {error}"
        ));
    }

    let mut bytes = Vec::new();
    let stop = loop {
        match drain(&mut stdout, &mut bytes, limit) {
            Drain::More => {}
            // A closed stream is the tool's whole answer; when the tool also
            // exited on its own, its status is the better report.
            Drain::Eof => {
                break match child.try_wait() {
                    Ok(Some(_)) => Stop::Exited,
                    _ => Stop::Eof,
                };
            }
            Drain::Over => break Stop::Over,
            Drain::Failed(detail) => break Stop::Failed(detail),
        }
        match child.try_wait() {
            Ok(Some(_)) => {
                // The tool may have written its last bytes as it exited; take
                // what is buffered and stop instead of waiting for a descendant.
                break match drain(&mut stdout, &mut bytes, limit) {
                    Drain::Over => Stop::Over,
                    Drain::Failed(detail) => Stop::Failed(detail),
                    Drain::More | Drain::Eof => Stop::Exited,
                };
            }
            Ok(None) => {}
            Err(error) => break Stop::Failed(format!("{program} could not be reaped: {error}")),
        }
        if control.cancel.load(Ordering::Acquire) {
            break Stop::Canceled;
        }
        let now = Instant::now();
        if now >= deadline {
            break Stop::Deadline;
        }
        wait_readable(stdout.as_raw_fd(), READ_POLL.min(deadline - now));
    };
    let exit = end_tool(&mut child, pid);
    control.pid.store(0, Ordering::Release);
    // Close our read end now: a descendant that inherited the write end must not
    // hold the descriptor open while we finish reporting.
    drop(stdout);

    match stop {
        Stop::Over => ToolRead::TooLarge,
        Stop::Deadline => ToolRead::Failed(format!(
            "the clipboard tool did not answer within {deadline_secs}s — press Ctrl-V to retry"
        )),
        Stop::Canceled => ToolRead::Failed("the clipboard read was canceled".to_string()),
        Stop::Failed(detail) => ToolRead::Failed(detail),
        Stop::Eof if !bytes.is_empty() => ToolRead::Bytes(bytes),
        Stop::Eof => ToolRead::Empty(format!(
            "{program} closed its output without data — the clipboard may hold no image"
        )),
        Stop::Exited if !bytes.is_empty() => ToolRead::Bytes(bytes),
        Stop::Exited => match exit {
            Some(exit) if exit.success() => ToolRead::Empty(format!(
                "{program} exited with no data — the clipboard may hold no image"
            )),
            Some(exit) => ToolRead::Empty(format!(
                "{program} exited with {exit} — the clipboard may hold no image or the session \
                 clipboard is unreachable"
            )),
            None => ToolRead::Empty(format!("{program} exited without a reapable status")),
        },
    }
}

/// What one non-blocking drain pass found.
enum Drain {
    /// Nothing more is ready (or some bytes were taken): keep polling.
    More,
    /// Every writer closed the pipe: the tool's answer is complete.
    Eof,
    /// The answer passed the caller's limit.
    Over,
    /// stdout could not be read: report the error, never a partial payload.
    Failed(String),
}

/// Take everything the tool has already written, up to one byte past `limit`.
/// Never blocks: the descriptor is non-blocking, so a descendant holding the
/// write end open looks like an ordinary "nothing ready".
fn drain(stdout: &mut ChildStdout, bytes: &mut Vec<u8>, limit: usize) -> Drain {
    let mut buffer = [0u8; 16 * 1024];
    loop {
        if bytes.len() > limit {
            return Drain::Over;
        }
        let want = ((limit + 1) - bytes.len()).min(buffer.len());
        match stdout.read(&mut buffer[..want]) {
            Ok(0) => return Drain::Eof,
            Ok(read) => {
                bytes.extend_from_slice(&buffer[..read]);
                if bytes.len() > limit {
                    return Drain::Over;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Drain::More,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => {
                return Drain::Failed(format!("clipboard tool stdout could not be read: {error}"));
            }
        }
    }
}

/// End the tool's process group and reap the direct child. Killing only the
/// parent would leave a descendant alive and holding our pipe.
fn end_tool(child: &mut Child, pid: u32) -> Option<std::process::ExitStatus> {
    // SAFETY: killpg takes integers only, and the tool leads its own group, so no
    // unrelated process is signalled. An already-empty group yields ESRCH, which
    // needs no handling.
    unsafe { libc::killpg(pid as i32, libc::SIGKILL) };
    child.wait().ok()
}

/// Make the tool's stdout non-blocking so a read pass can never hang.
fn set_nonblocking(fd: RawFd) -> std::io::Result<()> {
    // SAFETY: fcntl on a descriptor this process owns; no pointers involved.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: as above; the call only updates the descriptor's status flags.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Wait for output, cancellation, or the poll interval. The wait is always
/// bounded, so a silent tool still honors the deadline and a cancel.
fn wait_readable(fd: RawFd, timeout: Duration) {
    let mut descriptors = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let millis = timeout.as_millis().min(i32::MAX as u128) as i32;
    // SAFETY: one initialized descriptor for a pipe this process owns; poll only
    // writes `revents` back into it.
    let _ = unsafe { libc::poll(&mut descriptors, 1, millis) };
}

fn spawn_error(program: &str, error: std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::NotFound {
        format!("{program} is not installed or not on PATH — press Ctrl-V after installing it")
    } else {
        format!("could not run {program}: {error}")
    }
}

fn backend() -> Result<Backend, String> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty());
    let x11 = std::env::var_os("DISPLAY").is_some_and(|value| !value.is_empty());
    select_backend(wayland, x11, on_path("wl-paste"), on_path("xclip"))
}

/// Wayland wins when both sessions are advertised: that is the display the
/// cockpit itself is drawn on, and XWayland's copy of a compositor selection can
/// lag the real one.
fn select_backend(
    wayland: bool,
    x11: bool,
    wl_paste: bool,
    xclip: bool,
) -> Result<Backend, String> {
    if wayland && wl_paste {
        return Ok(Backend::Wayland);
    }
    if x11 && xclip {
        return Ok(Backend::X11);
    }
    let mut missing = Vec::new();
    if wayland && !wl_paste {
        missing.push("wl-clipboard (wl-paste) for this Wayland session");
    }
    if x11 && !xclip {
        missing.push("xclip for this DISPLAY");
    }
    Err(if missing.is_empty() {
        "no clipboard backend: this session exposes neither WAYLAND_DISPLAY nor DISPLAY".to_string()
    } else {
        format!("no clipboard backend: install {}", missing.join(" or "))
    })
}

/// PATH lookup without spawning a shell (`which` is not guaranteed present).
fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// True while the PID is a live process (a zombie is not alive). Shared with the
/// paste suites so cancellation and descendant cleanup can be observed without
/// touching the desktop clipboard.
#[cfg(test)]
pub(crate) fn process_is_running(pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    stat.rsplit_once(')')
        .and_then(|(_, rest)| rest.split_whitespace().next())
        != Some("Z")
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/clipboard__tests.rs"]
mod tests;
