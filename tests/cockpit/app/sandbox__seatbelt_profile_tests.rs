use super::*;

#[test]
fn seatbelt_profile_mirrors_the_landlock_posture() {
    let policy = SandboxPolicy {
        writable_roots: vec![PathBuf::from("/angel-nonexistent-root/work \"space\"")],
        allow_network: false,
        enforce: true,
        mandatory: false,
        sealed_reads: Vec::new(),
        deny_reads: Vec::new(),
    };
    let profile = seatbelt_profile(&policy);
    let lines: Vec<&str> = profile.lines().collect();
    // Order carries the semantics: SBPL is last-match-wins, so the broad
    // read-everything / deny-writes pair must precede every allowance, and
    // the network deny must come last.
    assert_eq!(lines[0], "(version 1)");
    assert_eq!(lines[1], "(allow default)");
    assert_eq!(lines[2], "(deny file-write*)");
    assert_eq!(lines.last(), Some(&"(deny network*)"));
    assert!(profile.contains(r#"(allow file-write* (literal "/dev/null"))"#));
    // A nonexistent root passes through verbatim (canonicalize fallback)
    // with SBPL string escaping applied to the embedded quote.
    assert!(
        profile
            .contains(r#"(allow file-write* (subpath "/angel-nonexistent-root/work \"space\""))"#),
        "{profile}"
    );

    let open = seatbelt_profile(&SandboxPolicy {
        writable_roots: Vec::new(),
        allow_network: true,
        enforce: true,
        mandatory: false,
        sealed_reads: Vec::new(),
        deny_reads: Vec::new(),
    });
    assert!(
        !open.contains("network"),
        "permissive network must not emit a deny: {open}"
    );
}

/// A command started through the helper must not reach the caller's terminal.
/// In the cockpit that terminal is the TUI: a password, host-key or credential
/// prompt would draw over it through /dev/tty and then stop on SIGTTIN until
/// the tool timeout. Linux gets the same from the helper's setsid.
#[cfg(target_os = "macos")]
#[test]
fn helper_command_cannot_open_the_callers_terminal() {
    use std::os::unix::process::CommandExt as _;
    let (mut master, mut slave) = (-1, -1);
    // SAFETY: two local out-pointers; no name, termios or window size.
    let opened = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(opened, 0, "openpty: {}", std::io::Error::last_os_error());
    // A session leader that holds the pty as its controlling terminal runs the
    // helper as an ordinary member of its session, as the cockpit does. The
    // trailing exit keeps sh from exec'ing the helper as the leader itself.
    let mut session = Command::new("/bin/sh");
    session
        .arg("-c")
        .arg(
            r#"(: </dev/tty) || exit 90
"$0" --sandbox-exec -- /bin/sh -c 'if (: </dev/tty) 2>/dev/null; then echo terminal; else echo detached; fi'
exit $?"#,
        )
        .arg(helper_executable().unwrap())
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    set_helper_policy(
        &mut session,
        &SandboxPolicy {
            writable_roots: Vec::new(),
            allow_network: true,
            enforce: false,
            mandatory: false,
            sealed_reads: Vec::new(),
            deny_reads: Vec::new(),
        },
    )
    .unwrap();
    // SAFETY: setsid and ioctl are async-signal-safe and touch only the child.
    unsafe {
        session.pre_exec(move || {
            if libc::setsid() < 0 || libc::ioctl(slave, libc::TIOCSCTTY as libc::c_ulong, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let output = session.output().unwrap();
    // SAFETY: both descriptors came from openpty above and are closed once.
    unsafe {
        libc::close(master);
        libc::close(slave);
    }
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "detached");
}
