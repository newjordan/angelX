//! Remote glass: our own tmux, whose client is the glass window
//! (docs/REMOTE_GLASS.md).
//!
//! A cockpit session lives on a machine under a small session process of
//! its own ([`serve`]) and survives every window that looks at it. A window
//! on another machine attaches over SSH ([`attach`]): `ssh <host> angelX
//! --session-host <name>` finds the session, or starts it, and relays the
//! link between the SSH command's stdio and the session's socket. What
//! crosses is state (`angel_glass::remote::wire`), and the window draws it
//! with the same renderer a local cockpit has.

mod attach;
mod serve;
mod term;

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

pub(crate) use serve::{presence_tick, serve, serve_requested};

/// The session a bare attach goes to.
pub(crate) const DEFAULT_SESSION: &str = "main";

/// Where a machine keeps its sessions' sockets and logs.
pub(crate) fn dir() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME").ok_or_else(|| io::Error::other("no HOME"))?;
    let dir = PathBuf::from(home).join(".angelX").join("remote-glass");
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

pub(crate) fn socket(name: &str) -> io::Result<PathBuf> {
    Ok(dir()?.join(format!("{name}.sock")))
}

/// A session name: letters, digits, `.`, `_` and `-`, up to 32.
pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// `host`, `host:name` or `:name` (this machine). `local` and `localhost`
/// are this machine too, without SSH.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub host: Option<String>,
    pub name: String,
}

impl Target {
    pub(crate) fn parse(text: &str) -> io::Result<Self> {
        let (host, name) = match text.rsplit_once(':') {
            Some((host, name)) => (host, name),
            None => (text, DEFAULT_SESSION),
        };
        let name = if name.is_empty() { DEFAULT_SESSION } else { name };
        if !valid_name(name) {
            return Err(invalid("a session name is letters, digits, '.', '_' and '-', up to 32"));
        }
        let host = match host {
            "" | "local" | "localhost" => None,
            host if host.starts_with('-') || host.chars().any(|c| c.is_whitespace() || c == '\'') => {
                return Err(invalid("not a host"));
            }
            host => Some(host.to_string()),
        };
        Ok(Self { host, name: name.to_string() })
    }

    pub(crate) fn label(&self) -> String {
        format!("{}:{}", self.host.as_deref().unwrap_or("local"), self.name)
    }
}

fn invalid(why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, why.to_string())
}

fn usage() -> io::Error {
    invalid(
        "usage: angelX <host>[:<session>] | --attach <host>[:<session>] [--term] | --sessions [<host>] | --kill-session <host>:<session>",
    )
}

/// The remote-glass commands.
pub(crate) fn cli(args: &[OsString]) -> io::Result<()> {
    let args: Vec<String> = args
        .iter()
        .map(|a| a.to_str().map(str::to_string).ok_or_else(usage))
        .collect::<io::Result<_>>()?;
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["--session-host", name] if valid_name(name) => return serve::host_session(name),
        ["--attach", target] => attach::attach(&Target::parse(target)?, None),
        ["--attach", target, "--term"] => attach::attach(&Target::parse(target)?, Some(false)),
        ["--attach", target, "--glass"] => attach::attach(&Target::parse(target)?, Some(true)),
        ["--sessions"] => attach::sessions(None),
        ["--sessions", host] => attach::sessions(Target::parse(host)?.host.as_deref()),
        ["--kill-session", target] => attach::kill(&Target::parse(target)?),
        _ => Err(usage()),
    };
    // One line, as the rest of the launcher speaks.
    if let Err(error) = result {
        eprintln!("angelX: {error}");
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_name_a_host_and_a_session() {
        let t = |s: &str| Target::parse(s).unwrap();
        assert_eq!(t("gpu-host"), Target { host: Some("gpu-host".into()), name: "main".into() });
        assert_eq!(t("gpu-host:work"), Target { host: Some("gpu-host".into()), name: "work".into() });
        assert_eq!(t("me@10.0.0.2:x"), Target { host: Some("me@10.0.0.2".into()), name: "x".into() });
        assert_eq!(t(":x"), Target { host: None, name: "x".into() });
        assert_eq!(t("local"), Target { host: None, name: "main".into() });
        assert!(Target::parse("gpu-host:../x").is_err());
        assert!(Target::parse("-oProxyCommand=x").is_err());
        assert!(Target::parse("a b").is_err());
    }
}
