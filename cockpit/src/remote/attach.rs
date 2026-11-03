//! The window's side: attach to a session, list sessions, end one.

use std::io::{self, BufRead, Read};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use angel_glass::GridSize;
use angel_glass::remote::{Fail, Kind, Link, Outcome, Status, handshake, wire};

use super::Target;

/// The angelX to run on the far side: `ANGEL_REMOTE_ANGELX` when set (a
/// test checkout), else the first of `angelX` on its PATH, the fleet's
/// `~/.local/bin/angelX` and a checkout at `~/angelX`. `ANGEL_REMOTE_ENV`
/// (`NAME=value` words) is set for it there.
fn remote_command(args: &str) -> String {
    let chosen = std::env::var("ANGEL_REMOTE_ANGELX").unwrap_or_default();
    let env: String = std::env::var("ANGEL_REMOTE_ENV")
        .unwrap_or_default()
        .split_whitespace()
        .filter(|word| word.split_once('=').is_some_and(|(k, _)| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')))
        .map(|word| format!("{} ", quote(word)))
        .collect();
    let exec = if env.is_empty() { "exec ".to_string() } else { format!("exec env {env}") };
    format!(
        "p={}; [ -x \"$p\" ] || p=$(command -v angelX 2>/dev/null); \
         [ -x \"$p\" ] || p=\"$HOME/.local/bin/angelX\"; [ -x \"$p\" ] || p=\"$HOME/angelX/bin/angelX\"; \
         [ -x \"$p\" ] || {{ echo 'angelX: no angelX on this machine' >&2; exit 127; }}; {exec}\"$p\" {args}",
        quote(&chosen)
    )
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// The process carrying a link: `ssh -T <host> angelX --session-host
/// <name>`, or this binary's own `--session-host` for a session here.
fn spawn(target: &Target) -> io::Result<Child> {
    let mut command = match &target.host {
        Some(host) => {
            let mut c = Command::new("ssh");
            // No terminal on the far side: the link is binary. Keepalives
            // notice a dead link within seconds, so the window reconnects.
            c.args(["-T", "-o", "ServerAliveInterval=5", "-o", "ServerAliveCountMax=3", "-o", "ConnectTimeout=10"])
                .arg(host)
                .arg(remote_command(&format!("--session-host {}", target.name)));
            c
        }
        None => {
            let mut c = Command::new(self_exe()?);
            c.args(["--session-host", &target.name]);
            c
        }
    };
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
}

fn self_exe() -> io::Result<std::path::PathBuf> {
    if cfg!(target_os = "linux") {
        Ok("/proc/self/exe".into())
    } else {
        std::env::current_exe()
    }
}

/// The far side's stderr: echoed while connecting (a launcher building
/// there says so), and its last lines kept to explain a failure.
#[derive(Clone, Default)]
struct Said(Arc<Mutex<(Vec<String>, bool)>>);

impl Said {
    /// Lines are echoed only once connecting is slow (a build there): a
    /// quick failure is explained in one line instead.
    fn listen(&self, stderr: impl Read + Send + 'static, echo: bool) {
        let said = self.clone();
        said.0.lock().unwrap().1 = echo;
        let began = Instant::now();
        std::thread::spawn(move || {
            for line in io::BufReader::new(stderr).lines() {
                let Ok(line) = line else { return };
                let mut s = said.0.lock().unwrap();
                if s.1 && began.elapsed() > Duration::from_millis(1500) {
                    eprintln!("{line}");
                }
                s.0.push(line);
                if s.0.len() > 20 {
                    s.0.remove(0);
                }
            }
        });
    }

    fn quiet(&self) {
        self.0.lock().unwrap().1 = false;
    }

    fn last(&self) -> String {
        self.0.lock().unwrap().0.iter().rev().find(|l| !l.trim().is_empty()).cloned().unwrap_or_default()
    }
}

/// A link to the session, through its hello.
fn connect(target: &Target, kind: Kind, size: GridSize, have: Vec<wire::Hash>, echo: bool) -> Result<Link, Fail> {
    let mut child = spawn(target).map_err(|e| Fail::Fatal(format!("could not run ssh: {e}")))?;
    let said = Said::default();
    if let Some(stderr) = child.stderr.take() {
        said.listen(stderr, echo);
    }
    let reader = Box::new(child.stdout.take().expect("piped stdout"));
    let writer = Box::new(child.stdin.take().expect("piped stdin"));
    let mut link = Link { reader, writer, child: Some(child) };
    let who = whoami();
    match handshake(&mut link, kind, size, have, &who) {
        Ok(_) => {
            said.quiet();
            Ok(link)
        }
        Err(fail) => {
            // Let the far side finish saying why.
            if let Some(child) = link.child.as_mut() {
                let until = Instant::now() + Duration::from_secs(2);
                while Instant::now() < until && matches!(child.try_wait(), Ok(None)) {
                    std::thread::sleep(Duration::from_millis(20));
                }
                let _ = child.kill();
                let _ = child.wait();
            }
            std::thread::sleep(Duration::from_millis(50));
            Err(explain(fail, &said.last()))
        }
    }
}

/// A failed hello with what the far side said on stderr: an angelX that
/// does not know `--session-host` is too old, and says so in one line.
fn explain(fail: Fail, said: &str) -> Fail {
    let old = said.contains("unknown or trailing interactive argument")
        || said.contains("unknown option")
        || said.contains("unexpected argument");
    match fail {
        _ if old => Fail::Fatal("the angelX there is too old for remote glass; update it there (angelX update)".into()),
        _ if said.contains("no angelX on this machine") => Fail::Fatal("there is no angelX there".into()),
        Fail::Again(why) if !said.is_empty() && why.contains("closed") => Fail::Again(said.to_string()),
        other => other,
    }
}

fn whoami() -> String {
    let user = std::env::var("USER").unwrap_or_default();
    let host = std::fs::read_to_string("/etc/hostname").map(|h| h.trim().to_string()).unwrap_or_default();
    format!("{user}@{host}")
}

/// Whether to attach in a window: asked for, or the launcher's own rule
/// (a desktop, outside tmux and SSH).
fn window_wanted(asked: Option<bool>) -> bool {
    if let Some(asked) = asked {
        return asked;
    }
    match std::env::var("ANGEL_GLASS").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => {
            std::env::var_os("TMUX").is_none()
                && std::env::var_os("SSH_CONNECTION").is_none()
                && (std::env::var_os("WAYLAND_DISPLAY").is_some()
                    || std::env::var_os("DISPLAY").is_some()
                    || cfg!(target_os = "macos"))
        }
    }
}

/// Attach to `target`'s session (started when it has none by that name),
/// in a glass window or, with no display, in this terminal.
pub(super) fn attach(target: &Target, window: Option<bool>) -> io::Result<()> {
    let label = target.label();
    let window = window_wanted(window);
    let outcome = if window {
        let options = angel_glass::Options::from_environment();
        let size = GridSize { cols: options.columns, rows: options.rows, ..GridSize::default() };
        eprintln!("angelX: attaching to {label}…");
        let first = connect(target, Kind::Window, size, Vec::new(), true).map_err(|f| fail(&label, f))?;
        let again = target.clone();
        angel_glass::remote::attach_window(options, first, label.clone(), move |size, have| {
            connect(&again, Kind::Window, size, have, false)
        })
        .map_err(io::Error::other)?
    } else {
        let size = super::term::size();
        let first = connect(target, Kind::Terminal, size, Vec::new(), true).map_err(|f| fail(&label, f))?;
        let again = target.clone();
        super::term::attach(first, &label, move |size, have| connect(&again, Kind::Terminal, size, have, false))?
    };
    match outcome {
        Outcome::Detached => {
            let back = match &target.host {
                Some(host) if target.name == super::DEFAULT_SESSION => format!("angelX {host}"),
                Some(host) => format!("angelX {host}:{}", target.name),
                None => format!("angelX --attach :{}", target.name),
            };
            eprintln!("angelX: detached from {label}; it runs on ({back} to return)");
        }
        Outcome::Ended(why) => eprintln!("angelX: {label} ended: {why}"),
    }
    Ok(())
}

fn fail(label: &str, fail: Fail) -> io::Error {
    io::Error::other(format!("{label}: {fail}"))
}

/// `--sessions [host]`: the sessions on this machine, or on `host`.
pub(super) fn sessions(host: Option<&str>) -> io::Result<()> {
    if let Some(host) = host {
        return over_ssh(host, "--sessions");
    }
    let mut found: Vec<Status> = Vec::new();
    let mut names: Vec<String> = std::fs::read_dir(super::dir()?)?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".sock").map(str::to_string))
        .collect();
    names.sort();
    for name in names {
        match probe(&name, false) {
            Ok(status) => found.push(status),
            // Nothing listens: a session that died without cleaning up.
            Err(e) if matches!(e.kind(), io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound) => {
                let _ = std::fs::remove_file(super::socket(&name)?);
            }
            // Something listens but did not answer as this angelX would
            // (another version): say so and leave it be.
            Err(e) => println!("{name}: running, but it did not answer this angelX ({e}); it may be another version"),
        }
    }
    if found.is_empty() {
        println!("no sessions");
        return Ok(());
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    for s in found {
        let age = now.saturating_sub(s.started);
        let age = if age >= 3600 { format!("{}h{:02}m", age / 3600, age / 60 % 60) } else { format!("{}m", age / 60) };
        let windows = match s.clients {
            0 => "detached".to_string(),
            1 => "1 window".to_string(),
            n => format!("{n} windows"),
        };
        let knights: Vec<String> = s
            .presence
            .knights
            .iter()
            .map(|k| format!("{} ({}, {})", k.persona, k.family, format!("{:?}", k.status).to_lowercase()))
            .collect();
        let at = if s.presence.workspace.is_empty() { String::new() } else { format!(" in {}", s.presence.workspace) };
        let knights = if knights.is_empty() { String::new() } else { format!("; {}", knights.join(", ")) };
        println!("{}: {}x{}, up {age}, {windows}{at}, pid {}{knights}", s.name, s.size.cols, s.size.rows, s.pid);
    }
    Ok(())
}

/// `--kill-session [host:]name`.
pub(super) fn kill(target: &Target) -> io::Result<()> {
    if let Some(host) = &target.host {
        return over_ssh(host, &format!("--kill-session :{}", target.name));
    }
    probe(&target.name, true)?;
    println!("ended {}", target.name);
    Ok(())
}

fn over_ssh(host: &str, args: &str) -> io::Result<()> {
    let status = Command::new("ssh").args(["-T", host]).arg(remote_command(args)).status()?;
    if status.success() { Ok(()) } else { Err(io::Error::other(format!("{host}: {status}"))) }
}

/// Ask a session on this machine who it is, and maybe to end.
fn probe(name: &str, kill: bool) -> io::Result<Status> {
    let mut stream = std::os::unix::net::UnixStream::connect(super::socket(name)?)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    wire::write(&mut stream, &angel_glass::remote::wire::Up::Hello {
        version: wire::VERSION,
        kind: Kind::Probe,
        size: GridSize::default(),
        have: Vec::new(),
        who: whoami(),
    })?;
    let mut status = None;
    while let Some(down) = wire::read::<wire::Down>(&mut stream)? {
        match down {
            wire::Down::Status(s) => {
                status = Some(s);
                break;
            }
            wire::Down::Ended(why) => return Err(io::Error::other(why)),
            _ => {}
        }
    }
    let status = status.ok_or_else(|| io::Error::other("no answer"))?;
    if kill {
        wire::write(&mut stream, &angel_glass::remote::wire::Up::Kill)?;
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_angelx_is_named_in_one_line() {
        let f = explain(Fail::Again("the link closed before a session answered".into()), "angelX: unknown or trailing interactive argument");
        assert!(matches!(f, Fail::Fatal(ref s) if s.contains("too old")), "{f:?}");
        let f = explain(Fail::Again("the link closed before a session answered".into()), "ssh: connect to host x port 22: No route to host");
        assert!(matches!(f, Fail::Again(ref s) if s.contains("No route")), "{f:?}");
    }

    #[test]
    fn the_far_command_is_quoted() {
        assert_eq!(quote("a'b"), "'a'\\''b'");
        if std::env::var_os("ANGEL_REMOTE_ENV").is_none() {
            assert!(remote_command("--session-host main").ends_with("exec \"$p\" --session-host main"));
        }
    }
}
