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
    assert_eq!(lines[0], "(version 1)");
    assert_eq!(lines[1], "(allow default)");
    assert_eq!(lines[2], "(deny file-write*)");
    assert_eq!(lines.last(), Some(&"(deny network*)"));
    assert!(profile.contains(r#"(allow file-write* (literal "/dev/null"))"#));
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

#[cfg(target_os = "macos")]
#[test]
fn helper_command_cannot_open_the_callers_terminal() {
    use std::os::unix::process::CommandExt as _;
    let (mut master, mut slave) = (-1, -1);
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
    unsafe {
        session.pre_exec(move || {
            if libc::setsid() < 0 || libc::ioctl(slave, libc::TIOCSCTTY as libc::c_ulong, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let output = session.output().unwrap();
    unsafe {
        libc::close(master);
        libc::close(slave);
    }
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "detached");
}
