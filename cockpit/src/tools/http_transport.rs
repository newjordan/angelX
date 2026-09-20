//! Killable, streaming ureq transport. The child owns DNS/TLS/socket state; the
//! parent owns liveness and always kills/reaps the child before returning.
//! The 25 ms supervisor tick is cancellation grace, not a request/run cap.
use crate::sandbox::process_owner::{Child, OwnedCommandExt};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::io::{self, Read, Write};
use std::process::{ChildStdout, Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_millis(25);
const PREVIEW_BYTES: usize = 4096;

#[derive(Clone)]
pub(crate) struct Context {
    deadline: Option<Instant>,
    stall: Duration,
    cancelled: Arc<AtomicBool>,
}
impl Default for Context {
    fn default() -> Self {
        Self {
            deadline: None,
            stall: Duration::from_secs(crate::club::identity_stream_stall_secs()),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}
thread_local! { static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) }; }
pub(crate) fn context() -> Context {
    CONTEXT
        .with(|slot| slot.borrow().clone())
        .unwrap_or_default()
}
pub(crate) fn with_context<R>(context: Context, operation: impl FnOnce() -> R) -> R {
    struct Restore(Option<Context>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CONTEXT.with(|slot| slot.replace(self.0.take()));
        }
    }
    let _restore = Restore(CONTEXT.with(|slot| slot.replace(Some(context))));
    operation()
}
pub(crate) fn with_deadline<R>(deadline: Option<Instant>, operation: impl FnOnce() -> R) -> R {
    let mut ctx = context();
    ctx.deadline = deadline;
    with_context(ctx, operation)
}
/// Tighten a tool's network budget without extending its enclosing deadline.
pub(crate) fn with_timeout<R>(timeout: Duration, operation: impl FnOnce() -> R) -> R {
    let mut ctx = context();
    let deadline = Instant::now() + timeout;
    ctx.deadline = Some(ctx.deadline.map_or(deadline, |outer| outer.min(deadline)));
    with_context(ctx, operation)
}
pub(crate) fn with_cancel<R>(cancel: Option<&AtomicBool>, operation: impl FnOnce() -> R) -> R {
    let Some(cancel) = cancel else {
        return operation();
    };
    let ctx = context();
    ctx.cancelled
        .store(cancel.load(Ordering::Acquire), Ordering::Release);
    let done = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let ctx = &ctx;
        let done_ref = &done;
        let watcher = scope.spawn(move || {
            while !done_ref.load(Ordering::Acquire) {
                if cancel.load(Ordering::Acquire) {
                    ctx.cancelled.store(true, Ordering::Release);
                    break;
                }
                std::thread::park_timeout(TICK);
            }
        });
        struct Done<'a>(&'a AtomicBool, std::thread::Thread);
        impl Drop for Done<'_> {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
                self.1.unpark();
            }
        }
        let _done = Done(&done, watcher.thread().clone());
        with_context(ctx.clone(), operation)
    })
}

#[derive(Serialize, Deserialize)]
struct WireRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
    guarded: bool,
    redirects: u32,
}
#[derive(Serialize, Deserialize)]
struct Metadata {
    status: u16,
    headers: Vec<(String, String)>,
}

pub(crate) struct Request {
    request: ureq::Request,
    guarded: bool,
    redirects: u32,
}
pub(crate) fn request(method: &str, url: &str, guarded: bool, redirects: u32) -> Request {
    Request {
        request: ureq::request(method, url),
        guarded,
        redirects,
    }
}
impl Request {
    pub(crate) fn set(mut self, name: &str, value: &str) -> Self {
        self.request = self.request.set(name, value);
        self
    }
    pub(crate) fn query(mut self, name: &str, value: &str) -> Self {
        self.request = self.request.query(name, value);
        self
    }
    pub(crate) fn call(self) -> Result<Response, Error> {
        self.send(None)
    }
    pub(crate) fn send_string(self, body: &str) -> Result<Response, Error> {
        self.send(Some(body.to_string()))
    }
    pub(crate) fn send_json(self, body: impl Serialize) -> Result<Response, Error> {
        let body = serde_json::to_string(&body).map_err(|e| Error::Transport(e.to_string()))?;
        self.set("Content-Type", "application/json")
            .send(Some(body))
    }
    fn send(self, body: Option<String>) -> Result<Response, Error> {
        let wire = WireRequest {
            method: self.request.method().to_string(),
            url: self
                .request
                .request_url()
                .map_err(|e| Error::Transport(e.to_string()))?
                .as_url()
                .to_string(),
            headers: self
                .request
                .header_names()
                .into_iter()
                .filter_map(|name| {
                    self.request
                        .header(&name)
                        .map(|value| (name.clone(), value.to_string()))
                })
                .collect(),
            body,
            guarded: self.guarded,
            redirects: self.redirects,
        };
        let mut reader = Body::start(wire).map_err(|e| Error::Transport(e.to_string()))?;
        let (kind, data) = reader
            .frame()
            .map_err(|e| Error::Transport(e.to_string()))?;
        if kind != b'h' {
            return Err(Error::Transport(
                String::from_utf8_lossy(&data).into_owned(),
            ));
        }
        let metadata: Metadata =
            serde_json::from_slice(&data).map_err(|e| Error::Transport(e.to_string()))?;
        let response = Response {
            metadata,
            reader: Box::new(reader),
        };
        if response.status() >= 400 {
            Err(Error::Status(response.status(), response))
        } else {
            Ok(response)
        }
    }
}

pub(crate) enum Error {
    Status(u16, Response),
    Transport(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Status(code, _) => write!(f, "HTTP {code}"),
            Self::Transport(error) => f.write_str(error),
        }
    }
}
impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
pub(crate) struct Response {
    metadata: Metadata,
    reader: Box<Body>,
}
impl Response {
    pub(crate) fn status(&self) -> u16 {
        self.metadata.status
    }
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.metadata
            .headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
    pub(crate) fn content_type(&self) -> &str {
        self.header("content-type")
            .unwrap_or("text/plain")
            .split(';')
            .next()
            .unwrap_or("text/plain")
            .trim()
    }
    pub(crate) fn into_reader(self) -> Body {
        *self.reader
    }
    pub(crate) fn into_string(self) -> io::Result<String> {
        let mut bytes = Vec::new();
        self.reader.take(u64::MAX).read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
    pub(crate) fn into_json<T: serde::de::DeserializeOwned>(self) -> io::Result<T> {
        serde_json::from_reader(self.reader).map_err(io::Error::other)
    }
}

struct Progress {
    last: Instant,
    bytes: usize,
    preview: Vec<u8>,
    bound: Option<&'static str>,
    done: bool,
}
pub(crate) struct Body {
    stdout: ChildStdout,
    child: Arc<Mutex<Child>>,
    progress: Arc<Mutex<Progress>>,
    watcher: Option<std::thread::JoinHandle<()>>,
    started: Instant,
    pending: io::Cursor<Vec<u8>>,
    finished: bool,
    marker_read: bool,
}
impl Body {
    fn start(wire: WireRequest) -> io::Result<Self> {
        let started = Instant::now();
        let ctx = context();
        let mut cmd = Command::new(std::env::current_exe()?);
        #[cfg(not(test))]
        cmd.arg("--tool-http-helper");
        #[cfg(test)]
        cmd.args([
            "--exact",
            "tools::http_transport::tests::http_child_entry",
            "--nocapture",
        ])
        .env("ANGEL_T_HTTP_CHILD", "1");
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn_owned()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("HTTP helper stdout missing"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("HTTP helper stdin missing"))?;
        let child = Arc::new(Mutex::new(child));
        let progress = Arc::new(Mutex::new(Progress {
            last: started,
            bytes: 0,
            preview: Vec::new(),
            bound: None,
            done: false,
        }));
        let watcher = {
            let child = child.clone();
            let progress = progress.clone();
            std::thread::spawn(move || {
                loop {
                    let mut progress = progress.lock().unwrap();
                    if progress.done {
                        break;
                    }
                    let now = Instant::now();
                    let bound = if ctx.deadline.is_some_and(|deadline| now >= deadline) {
                        Some("deadline")
                    } else if ctx.cancelled.load(Ordering::Acquire) {
                        Some("cancel")
                    } else if !ctx.stall.is_zero() && now.duration_since(progress.last) >= ctx.stall
                    {
                        Some("stall")
                    } else {
                        None
                    };
                    if let Some(bound) = bound {
                        progress.bound = Some(bound);
                        let _ = child.lock().unwrap().kill();
                        break;
                    }
                    drop(progress);
                    std::thread::park_timeout(TICK);
                }
            })
        };
        let reader = Self {
            stdout,
            child,
            progress,
            watcher: Some(watcher),
            started,
            pending: io::Cursor::new(Vec::new()),
            finished: false,
            marker_read: false,
        };
        // The watchdog also covers a blocked request-body pipe write.
        serde_json::to_writer(stdin, &wire).map_err(|e| reader.error(io::Error::other(e)))?;
        Ok(reader)
    }
    fn error(&self, error: io::Error) -> io::Error {
        let progress = self.progress.lock().unwrap();
        if let Some(bound) = progress.bound {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "stalled {}",
                    serde_json::json!({"bytes_received": progress.bytes, "elapsed_ms": self.started.elapsed().as_millis(), "bound": bound, "partial_body": String::from_utf8_lossy(&progress.preview), "partial_bytes_base64": base64::engine::general_purpose::STANDARD.encode(&progress.preview), "partial_truncated": progress.bytes > progress.preview.len()})
                ),
            )
        } else {
            io::Error::new(
                error.kind(),
                format!(
                    "HTTP transport error {}",
                    serde_json::json!({"bytes_received": progress.bytes, "elapsed_ms": self.started.elapsed().as_millis(), "cause": error.to_string(), "partial_body": String::from_utf8_lossy(&progress.preview), "partial_bytes_base64": base64::engine::general_purpose::STANDARD.encode(&progress.preview), "partial_truncated": progress.bytes > progress.preview.len()})
                ),
            )
        }
    }
    fn frame(&mut self) -> io::Result<(u8, Vec<u8>)> {
        let mut header = [0; 5];
        // The Rust test harness writes a preamble; child entry emits a marker.
        if !self.marker_read {
            self.marker_read = true;
            let mut matched = 0;
            let marker = b"ANGEL_HTTP\n";
            while matched < marker.len() {
                let mut byte = [0];
                self.stdout
                    .read_exact(&mut byte)
                    .map_err(|e| self.error(e))?;
                matched = if byte[0] == marker[matched] {
                    matched + 1
                } else {
                    usize::from(byte[0] == marker[0])
                };
            }
        }
        self.stdout
            .read_exact(&mut header)
            .map_err(|e| self.error(e))?;
        let len = u32::from_be_bytes(header[1..].try_into().unwrap()) as usize;
        let mut data = vec![0; len];
        let mut received = 0;
        while received < len {
            let n = self
                .stdout
                .read(&mut data[received..])
                .map_err(|e| self.error(e))?;
            if n == 0 {
                return Err(self.error(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "HTTP helper closed mid-frame",
                )));
            }
            if header[0] == b'd' {
                let mut progress = self.progress.lock().unwrap();
                progress.last = Instant::now();
                progress.bytes += n;
                let keep = n.min(PREVIEW_BYTES.saturating_sub(progress.preview.len()));
                progress
                    .preview
                    .extend_from_slice(&data[received..received + keep]);
            }
            received += n;
        }
        let mut progress = self.progress.lock().unwrap();
        if progress.bound.is_some() {
            drop(progress);
            return Err(self.error(io::Error::other("HTTP request interrupted")));
        }
        if header[0] == b'h' {
            progress.last = Instant::now();
        }
        if header[0] == b'f' {
            progress.done = true;
        }
        Ok((header[0], data))
    }
}
impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let n = self.pending.read(buf)?;
        if n != 0 || self.finished {
            return Ok(n);
        }
        let (kind, data) = self.frame()?;
        match kind {
            b'd' => {
                self.pending = io::Cursor::new(data);
                self.pending.read(buf)
            }
            b'f' => {
                self.finished = true;
                Ok(0)
            }
            b'e' => Err(self.error(io::Error::other(
                String::from_utf8_lossy(&data).into_owned(),
            ))),
            _ => Err(io::Error::other("invalid HTTP helper frame")),
        }
    }
}
impl Drop for Body {
    fn drop(&mut self) {
        self.progress.lock().unwrap().done = true;
        if let Some(watcher) = self.watcher.take() {
            watcher.thread().unpark();
            let _ = watcher.join();
        }
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
    }
}
fn emit(out: &mut impl Write, kind: u8, data: &[u8]) -> io::Result<()> {
    out.write_all(&[kind])?;
    out.write_all(&(data.len() as u32).to_be_bytes())?;
    out.write_all(data)?;
    out.flush()
}
pub(crate) fn helper_main() -> io::Result<()> {
    let wire: WireRequest = serde_json::from_reader(io::stdin().lock())?;
    let mut out = io::stdout().lock();
    out.write_all(b"ANGEL_HTTP\n")?;
    out.flush()?;
    let mut builder = ureq::AgentBuilder::new().redirects(wire.redirects);
    if wire.guarded && !crate::yolo::enabled() {
        let private = crate::harness::env_flag("ANGEL_HTTP_ALLOW_PRIVATE_NETWORK", false);
        builder = builder
            .resolver(move |netloc: &str| crate::tools::web::resolve_http_target(netloc, private));
    }
    let agent = builder.build();
    let mut request = agent.request(&wire.method, &wire.url);
    for (name, value) in wire.headers {
        request = request.set(&name, &value);
    }
    let response = match wire.body {
        Some(body) => request.send_string(&body),
        None => request.call(),
    };
    let response = match response {
        Ok(response) | Err(ureq::Error::Status(_, response)) => response,
        Err(error) => return emit(&mut out, b'e', error.to_string().as_bytes()),
    };
    let metadata = Metadata {
        status: response.status(),
        headers: response
            .headers_names()
            .into_iter()
            .filter_map(|name| {
                response
                    .header(&name)
                    .map(|value| (name.clone(), value.to_string()))
            })
            .collect(),
    };
    emit(&mut out, b'h', &serde_json::to_vec(&metadata)?)?;
    let mut reader = response.into_reader();
    let mut chunk = [0; 8192];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => return emit(&mut out, b'f', &[]),
            Ok(n) => emit(&mut out, b'd', &chunk[..n])?,
            Err(error) => return emit(&mut out, b'e', error.to_string().as_bytes()),
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/tools/http_transport__tests.rs"]
pub(crate) mod tests;
