//! The session's side: the far end of an attach (`--session-host`) and the
//! session process itself (`serve`).

use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use angel_glass::GridSize;
use angel_glass::remote::{Knight, KnightStatus, Presence, wire};

const SERVE_ENV: &str = "ANGEL_SESSION_SERVE";

pub(crate) fn serve_requested() -> Option<String> {
    std::env::var(SERVE_ENV).ok().filter(|name| super::valid_name(name))
}

pub(crate) fn host_session(name: &str) -> io::Result<()> {
    let session = connect_or_start(name)?;
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
    std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(fd) })
}

pub(crate) fn connect_or_start(name: &str) -> io::Result<UnixStream> {
    let socket = super::socket(name)?;
    if let Ok(stream) = UnixStream::connect(&socket) {
        return Ok(stream);
    }
    let lock_path = super::dir()?.join(format!("{name}.lock"));
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&lock_path)?;
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

fn start(name: &str) -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    let exe = if cfg!(target_os = "linux") {
        PathBuf::from("/proc/self/exe")
    } else {
        std::env::current_exe()?
    };
    let log = std::fs::OpenOptions::new().create(true).append(true).open(log(name)?)?;
    let mut command = std::process::Command::new(exe);
    command
        .arg0(format!("angelX-session:{name}"))
        .env(SERVE_ENV, name)
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
    eprintln!("=== {} session {name} pid {} ===", chrono_now(), std::process::id());
    let size = GridSize { cols: 120, rows: 40, ..GridSize::default() };
    let (host, hub) = angel_glass::remote::headless(name, size);
    {
        let hub = hub.clone();
        std::thread::Builder::new().name("remote-accept".into()).spawn(move || {
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
    let deadline = Instant::now() + Duration::from_secs(10);
    while (hub.status().clients == 0 || hub.status().size.cell_width == 0) && Instant::now() < deadline {
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
        Ok(Some(reborn)) => crate::phoenix(Ok(Some(reborn))),
        other => {
            hub.end("the cockpit quit");
            other.map(|_| ())
        }
    }
}

static HUB: std::sync::OnceLock<(angel_glass::remote::Hub, String)> = std::sync::OnceLock::new();

fn presence(hub: angel_glass::remote::Hub, workspace: PathBuf) {
    let workspace = workspace.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    hub.set_presence(Presence { workspace: workspace.clone(), ..Presence::default() });
    let _ = HUB.set((hub, workspace));
}

pub(crate) fn presence_tick(app: &crate::app::App) {
    use std::sync::Mutex;
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let Some((hub, workspace)) = HUB.get() else { return };
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_some_and(|at| at.elapsed() < Duration::from_secs(1)) {
            return;
        }
        *last = Some(Instant::now());
    }
    let knights = serving_knights(app);
    hub.set_presence(Presence { workspace: workspace.clone(), knights, ..Presence::default() });
}

fn serving_knights(app: &crate::app::App) -> Vec<Knight> {
    use angel_plug::drive::DriveWorld;
    let turn = app.thinking.is_some();
    let (label, driver, model) = match app.thinking.as_ref() {
        Some(thinking) => {
            let resolved = thinking
                .club
                .as_ref()
                .and_then(|club| club.resolved_route_if_known());
            let route = resolved.as_ref().unwrap_or(&thinking.requested_route);
            (thinking.club_label.clone(), route.driver.clone(), route.model.clone())
        }
        None => {
            let chrome = app.bag.in_hand_chrome();
            (
                app.bag.in_hand_label().to_string(),
                chrome.route.driver.clone(),
                chrome.route.model.clone(),
            )
        }
    };
    let seats: Vec<(String, String, String)> = app
        .moa_armed()
        .into_iter()
        .flat_map(|engagement| engagement.roster.assignments().iter().flatten())
        .map(|seat| (seat.agent.clone(), seat.driver.clone(), seat.model.clone()))
        .collect();
    let key_of = crate::ui::agent_panel::profile::profile_for_route;
    let seats: Vec<_> = seats
        .iter()
        .map(|(agent, driver, model)| key_of(agent, driver, Some(model)).key)
        .collect();
    app.realm
        .serving_knights(key_of(&label, &driver, model.as_deref()).key, &seats)
        .into_iter()
        .enumerate()
        .map(|(i, (family, persona))| Knight {
            family,
            persona,
            doing: if turn && i == 0 { "working".into() } else { String::new() },
            status: match (turn, i) {
                (true, 0) => KnightStatus::Working,
                (true, _) => KnightStatus::Waiting,
                _ => KnightStatus::Idle,
            },
            task: String::new(),
        })
        .collect()
}

fn chrono_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("unix {secs}")
}
