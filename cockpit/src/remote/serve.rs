//! The session's side: the far end of an attach (`--session-host`) and the
//! session process itself (`serve`).

use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use angel_glass::GridSize;
use angel_glass::remote::{Presence, wire};

/// The environment variable that makes a launch a session's process, naming
/// the session.
const SERVE_ENV: &str = "ANGEL_SESSION_SERVE";

/// The session this process is to serve, when it is one.
pub(crate) fn serve_requested() -> Option<String> {
    std::env::var(SERVE_ENV)
        .ok()
        .filter(|name| super::valid_name(name))
}

/// The far end of an attach, run by `ssh <host> angelX --session-host
/// <name>`: find the session or start it, then relay the link between this
/// process's stdin and stdout and the session's socket until either side
/// closes. The session outlives it.
pub(crate) fn host_session(name: &str) -> io::Result<()> {
    let session = connect_or_start(name)?;
    // Unbuffered stdout: a frame must leave when it is written, not when a
    // line ends.
    let mut out = raw_fd(1);
    out.write_all(wire::MAGIC)?;
    out.flush()?;
    let mut to_session = session.try_clone()?;
    std::thread::spawn(move || {
        let mut input = raw_fd(0);
        let _ = pump(&mut *input, &mut to_session);
        let _ = to_session.shutdown(std::net::Shutdown::Write);
    });
    let mut from_session = session;
    let _ = pump(&mut from_session, &mut *out);
    Ok(())
}

/// Copy until either side closes, writing each read at once.
fn pump(from: &mut impl Read, to: &mut impl Write) -> io::Result<()> {
    let mut buffer = vec![0u8; 256 << 10];
    loop {
        let n = match from.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        to.write_all(&buffer[..n])?;
        to.flush()?;
    }
}

fn raw_fd(fd: i32) -> std::mem::ManuallyDrop<std::fs::File> {
    use std::os::fd::FromRawFd;
    // The standard streams stay open for the process's life.
    std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(fd) })
}

/// The session's socket, starting the session when none answers.
pub(crate) fn connect_or_start(name: &str) -> io::Result<UnixStream> {
    let socket = super::socket(name)?;
    if let Ok(stream) = UnixStream::connect(&socket) {
        return Ok(stream);
    }
    // One starter at a time: two windows attaching at once start one session.
    let lock_path = super::dir()?.join(format!("{name}.lock"));
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)?;
    use std::os::fd::AsRawFd;
    unsafe {
        libc::flock(lock.as_raw_fd(), libc::LOCK_EX);
    }
    if let Ok(stream) = UnixStream::connect(&socket) {
        return Ok(stream);
    }
    start(name)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match UnixStream::connect(&socket) {
            Ok(stream) => return Ok(stream),
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            Err(e) => {
                return Err(io::Error::other(format!(
                    "the session {name} did not start ({e}); see {}",
                    log(name)?.display()
                )));
            }
        }
    }
}

fn log(name: &str) -> io::Result<PathBuf> {
    Ok(super::dir()?.join(format!("{name}.err")))
}

/// Start the session's process: this same binary, in a session of its own
/// (no terminal, no SSH connection it could die with), its stderr in the
/// session's log.
fn start(name: &str) -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    // The launcher runs a private link to the binary and removes it when it
    // exits; /proc/self/exe stays good after that.
    let exe = if cfg!(target_os = "linux") {
        PathBuf::from("/proc/self/exe")
    } else {
        std::env::current_exe()?
    };
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log(name)?)?;
    let mut command = std::process::Command::new(exe);
    command
        // `ps` names it for what it is.
        .arg0(format!("angelX-session:{name}"))
        .env(SERVE_ENV, name)
        // Its children (the shell pane) are not windows.
        .env("ANGEL_GLASS", "0")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(log);
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    command.spawn()?;
    Ok(())
}

/// The session's process: the cockpit under a host with no window, and a
/// socket where windows (and `--sessions`) find it.
pub(crate) fn serve(
    name: &str,
    launch: crate::interactive_launch::InteractiveLaunch,
    workspace: PathBuf,
) -> io::Result<()> {
    let socket = super::socket(name)?;
    let _ = std::fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
    }
    eprintln!(
        "=== {} session {name} pid {} ===",
        chrono_now(),
        std::process::id()
    );
    // No cell pixels until a window says (its window opens after its hello).
    let size = GridSize {
        cols: 120,
        rows: 40,
        ..GridSize::default()
    };
    let (host, hub) = angel_glass::remote::headless(name, size);
    {
        let hub = hub.clone();
        std::thread::Builder::new()
            .name("remote-accept".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { continue };
                    let hub = hub.clone();
                    std::thread::spawn(move || {
                        let reader = match stream.try_clone() {
                            Ok(reader) => reader,
                            Err(_) => return,
                        };
                        if let Err(e) = hub.serve(reader, stream) {
                            eprintln!("REMOTE: a link failed: {e}");
                        }
                    });
                }
            })?;
    }
    // The first window sizes the cockpit, cells and pixels, before it draws.
    let deadline = Instant::now() + Duration::from_secs(10);
    while (hub.status().clients == 0 || hub.status().size.cell_width == 0)
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    hub.default_cell((10, 20));
    presence(hub.clone(), workspace.clone());
    crate::glass_drawing();
    let cockpit = std::thread::Builder::new()
        .name("cockpit".into())
        .stack_size(64 << 20)
        .spawn(move || crate::cockpit_on_host(host, launch, workspace))?;
    let result = match cockpit.join() {
        Ok(result) => result,
        Err(panic) => {
            hub.end("the cockpit stopped unexpectedly");
            let _ = std::fs::remove_file(&socket);
            std::panic::resume_unwind(panic);
        }
    };
    let _ = std::fs::remove_file(&socket);
    match result {
        // `/self reborn`: the new binary takes the session over; windows
        // see their link drop and attach again.
        Ok(Some(reborn)) => crate::phoenix(Ok(Some(reborn))),
        other => {
            hub.end("the cockpit quit");
            other.map(|_| ())
        }
    }
}

/// The session's hub, for the cockpit's loop to say who is at work.
static HUB: std::sync::OnceLock<(angel_glass::remote::Hub, String)> = std::sync::OnceLock::new();

/// Who is at work in the session, for kingdoms watching it
/// (docs/REMOTE_GLASS.md, "Presence").
fn presence(hub: angel_glass::remote::Hub, workspace: PathBuf) {
    let workspace = workspace
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    hub.set_presence(Presence {
        workspace: workspace.clone(),
        ..Presence::default()
    });
    let _ = HUB.set((hub, workspace));
}

/// After each drawn frame, at most once a second: the serving houses'
/// knights as the session's presence. Nothing outside a session.
pub(crate) fn presence_tick(app: &crate::app::App) {
    use angel_glass::remote::{Knight, KnightStatus};
    use std::sync::Mutex;
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let Some((hub, workspace)) = HUB.get() else {
        return;
    };
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_some_and(|at| at.elapsed() < Duration::from_secs(1)) {
            return;
        }
        *last = Some(Instant::now());
    }
    let serving = app.serving_houses();
    let knights = serving
        .houses()
        .enumerate()
        .map(|(i, id)| {
            let house = crate::stage::houses::get(id);
            Knight {
                family: house.key.to_string(),
                persona: house
                    .knight(0)
                    .map(|k| k.name.clone())
                    .unwrap_or_else(|| house.label()),
                doing: if serving.turn && i == 0 {
                    "working".into()
                } else {
                    String::new()
                },
                status: match (serving.turn, i) {
                    (true, 0) => KnightStatus::Working,
                    (true, _) => KnightStatus::Waiting,
                    _ => KnightStatus::Idle,
                },
                task: String::new(),
            }
        })
        .collect();
    hub.set_presence(Presence {
        workspace: workspace.clone(),
        knights,
        ..Presence::default()
    });
}

fn chrono_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("unix {secs}")
}
