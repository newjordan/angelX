//! Local host diagnostics. Probes never run a candidate or contact a provider.
use crate::sandbox::process_owner::OwnedCommandExt;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cause {
    AppArmor,
    UsernsDisabled,
    MissingBwrap,
    MissingUserns,
    NamespaceDenied,
}

impl Cause {
    pub(crate) fn class(self) -> &'static str {
        match self {
            Self::AppArmor => "userns denied by AppArmor",
            Self::UsernsDisabled => "unprivileged_userns_clone=0",
            Self::MissingBwrap => "missing bwrap",
            Self::MissingUserns => "missing /proc/self/ns/user",
            Self::NamespaceDenied => "user namespace unavailable",
        }
    }

    pub(crate) fn remedy(self) -> &'static str {
        match self {
            Self::AppArmor => {
                "sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0; persist with: printf '%s\\n' 'kernel.apparmor_restrict_unprivileged_userns=0' | sudo tee /etc/sysctl.d/90-angel-userns.conf"
            }
            Self::UsernsDisabled => {
                "sudo sysctl -w kernel.unprivileged_userns_clone=1; persist with: printf '%s\\n' 'kernel.unprivileged_userns_clone=1' | sudo tee /etc/sysctl.d/90-angel-userns.conf"
            }
            Self::MissingBwrap => "sudo apt install bubblewrap",
            Self::MissingUserns => "boot a kernel with CONFIG_USER_NS=y and mount procfs at /proc",
            Self::NamespaceDenied => {
                "allow unprivileged user namespaces in the host/container security policy; run angel --doctor after the host fix"
            }
        }
    }

    pub(crate) fn notice(self) -> String {
        format!(
            "sandbox: bwrap unavailable ({}) → Landlock-only confinement",
            self.class()
        )
    }
}

pub(crate) fn classify(
    bwrap: bool,
    userns: bool,
    apparmor: bool,
    clone_disabled: bool,
    stderr: &str,
) -> Option<Cause> {
    if !bwrap {
        return Some(Cause::MissingBwrap);
    }
    if !userns {
        return Some(Cause::MissingUserns);
    }
    if clone_disabled {
        return Some(Cause::UsernsDisabled);
    }
    let denied = stderr.contains("Permission denied")
        || stderr.contains("Operation not permitted")
        || stderr.contains("namespace failed")
        || stderr.contains("No permissions to create new namespace");
    if denied && apparmor {
        Some(Cause::AppArmor)
    } else if denied || !stderr.is_empty() {
        Some(Cause::NamespaceDenied)
    } else {
        None
    }
}

/// One probe per process. The bwrap trial (`--unshare-user --ro-bind / /`) is a
/// full namespace + mount setup; running it on every confined tool call and at
/// each start-up made start-up erratic (0.2 s → 3–6 s under load, batch-24 pin).
/// The host facts it classifies do not change while the cockpit runs.
pub(crate) fn detect() -> Option<Cause> {
    static CACHED: std::sync::OnceLock<Option<Cause>> = std::sync::OnceLock::new();
    *CACHED.get_or_init(detect_uncached)
}

fn detect_uncached() -> Option<Cause> {
    let bwrap = Path::new("/usr/bin/bwrap").is_file();
    let userns = Path::new("/proc/self/ns/user").exists();
    let setting = |name: &str| {
        std::fs::read_to_string(name)
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let apparmor = setting("/proc/sys/kernel/apparmor_restrict_unprivileged_userns") == "1";
    let clone_disabled = setting("/proc/sys/kernel/unprivileged_userns_clone") == "0";
    if let Some(cause) = classify(bwrap, userns, apparmor, clone_disabled, "") {
        return Some(cause);
    }
    // A minimal trial: user namespace + an empty root is enough to learn whether
    // the uid map can be set up; binding the whole host root is not needed and
    // is what made the probe slow. Bounded so a wedged helper never delays
    // start-up: no verdict within the bound means "not denied".
    let mut child = match std::process::Command::new("/usr/bin/bwrap")
        .args([
            "--unshare-user",
            "--ro-bind",
            "/usr",
            "/usr",
            "--symlink",
            "usr/bin",
            "/bin",
            "--symlink",
            "usr/lib",
            "/lib",
            "--symlink",
            "usr/lib64",
            "/lib64",
            "--",
            "/usr/bin/true",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn_owned()
    {
        Ok(child) => child,
        Err(_) => return Some(Cause::NamespaceDenied),
    };
    let started = std::time::Instant::now();
    let bound = std::time::Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stderr = String::new();
                if let Some(mut pipe) = child.stderr.take() {
                    use std::io::Read;
                    let _ = pipe.read_to_string(&mut stderr);
                }
                return if status.success() {
                    None
                } else {
                    classify(bwrap, userns, apparmor, clone_disabled, &stderr)
                        .or(Some(Cause::NamespaceDenied))
                };
            }
            Ok(None) if started.elapsed() < bound => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                eprintln!(
                    "[sandbox] compatibility probe exceeded {} ms; assuming namespaces work",
                    bound.as_millis()
                );
                return None;
            }
            Err(_) => return Some(Cause::NamespaceDenied),
        }
    }
}

pub(crate) fn doctor() -> String {
    match detect() {
        Some(cause) => format!(
            "sandbox: bwrap unavailable ({}) — {}\nHost remedy: {}",
            cause.class(),
            if super::available() {
                "Landlock-only confinement available"
            } else {
                "Landlock unavailable; execution blocked until host repair"
            },
            cause.remedy(),
        ),
        None => format!(
            "sandbox: Bubblewrap user namespaces available; Landlock available: {}",
            super::available()
        ),
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/sandbox__compatibility__tests.rs"]
mod tests;
