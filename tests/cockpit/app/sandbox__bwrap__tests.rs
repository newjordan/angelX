use super::super as sandbox;
use super::*;
use std::path::PathBuf;

fn policy(root: PathBuf, network: bool) -> SandboxPolicy {
    SandboxPolicy {
        writable_roots: vec![root],
        allow_network: network,
        enforce: true,
        mandatory: true,
        sealed_reads: Vec::new(),
        deny_reads: Vec::new(),
    }
}

#[test]
fn missing_bwrap_fails_closed_before_launch() {
    let error = command(
        Path::new("/angel-no-such-bwrap"),
        &policy(std::env::temp_dir(), true),
        "true".into(),
        Vec::new(),
        false,
    )
    .unwrap_err();
    assert!(error.to_string().contains("refusing unconfined fallback"));
}

#[test]
fn kernel_write_roots_are_rejected() {
    let executable = std::env::current_exe().unwrap();
    for root in ["/", "/proc", "/sys"] {
        let error = command(
            &executable,
            &policy(root.into(), true),
            "true".into(),
            Vec::new(),
            false,
        )
        .unwrap_err();
        assert!(error.to_string().contains("rejects host kernel write root"));
    }
}

#[test]
fn source_pin_rejects_intermediate_symlink_substitution() {
    let base = std::env::temp_dir().join(format!("angel_bwrap_pin_{}", std::process::id()));
    std::fs::create_dir_all(base.join("real")).unwrap();
    std::fs::write(base.join("real/file"), "safe").unwrap();
    std::os::unix::fs::symlink(base.join("real"), base.join("swapped")).unwrap();
    assert!(pin_source(&base.join("real/file")).is_ok());
    assert!(pin_source(&base.join("swapped/file")).is_err());
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn bwrap_confines_nested_verifiers_and_denies_escape_routes() {
    let _lock = crate::tests::env_lock();
    let probe = Command::new(EXECUTABLE)
        .args(["--unshare-user", "--ro-bind", "/", "/", "--", "/bin/true"])
        .output();
    if !probe.as_ref().is_ok_and(|output| output.status.success()) {
        assert_ne!(
            std::env::var("ANGEL_BWRAP_STRICT").as_deref(),
            Ok("1"),
            "strict bwrap suite needs a namespace-capable host: {probe:?}"
        );
        eprintln!("bwrap unavailable or host namespace setup denied; skipping integration check");
        return;
    }
    let base = std::env::temp_dir().join(format!("angel_bwrap_contract_{}", std::process::id()));
    let allowed = base.join("allowed");
    let denied = base.join("denied");
    std::fs::create_dir_all(&allowed).unwrap();
    std::fs::create_dir_all(&denied).unwrap();
    std::fs::write(denied.join("sentinel"), "unchanged").unwrap();
    std::os::unix::fs::symlink(&denied, allowed.join("escape")).unwrap();
    let script = r#"
import ctypes, os, pathlib, socket, subprocess, sys
allowed, denied = map(pathlib.Path, sys.argv[1:3])
try: os.write(int(sys.argv[3]), b'escaped through inherited fd')
except OSError as error: assert error.errno == 9
else: raise AssertionError('unapproved writable descriptor reached payload')
for name in os.listdir('/proc/self/fd'):
 if int(name) <= 2: continue
 try: os.fstat(int(name))
 except OSError: continue
 raise AssertionError('mount-source or inherited descriptor reached payload')
(allowed/'approved').write_text('yes')
def must_deny(operation):
 try: operation()
 except OSError: return
 raise AssertionError('operation escaped confinement')
must_deny(lambda: (denied/'outside').write_text('bad'))
must_deny(lambda: (allowed/'escape'/'symlink').write_text('bad'))
must_deny(lambda: os.rename(allowed/'approved', denied/'renamed'))
must_deny(lambda: os.chmod(denied/'sentinel', 0o777))
for name in ('null','zero','full','random','urandom','tty'): assert os.path.exists('/dev/'+name), name
assert open('/dev/zero','rb').read(4) == b'\0'*4 and len(open('/dev/urandom','rb').read(4)) == 4
must_deny(lambda: os.write(os.open('/dev/full', os.O_WRONLY), b'x'))
assert not any(os.path.exists('/dev/'+name) for name in ('sda','nvme0n1','vda','mem','kmsg','pts/0','shm','console')), sorted(os.listdir('/dev'))
libc=ctypes.CDLL(None, use_errno=True)
assert libc.mount(None, b'/', None, 32, None) != 0, 'outer root remount escaped'
nested = subprocess.run(['/usr/bin/bwrap','--unshare-user','--unshare-pid','--ro-bind','/','/','--proc','/proc','--dev','/dev','--','/usr/bin/python3','-c',
 'import ctypes; assert ctypes.CDLL(None).mount(None,b"/",None,32,None) != 0; assert open("/dev/zero","rb").read(1) == bytes(1)'], capture_output=True, text=True, timeout=5)
assert nested.returncode == 0, nested.stderr
assert (denied/'sentinel').read_text() == 'unchanged'
status=pathlib.Path('/proc/self/status').read_text()
assert 'NoNewPrivs:\t1' in status
assert 'CapEff:\t0000000000000000' in status
assert os.getsid(0) != os.getsid(os.getppid()) or os.getppid() == 1
print('nested-and-confinement-ok')
"#;
    let inherited = std::fs::OpenOptions::new()
        .write(true)
        .open(denied.join("sentinel"))
        .unwrap();
    assert_ne!(
        unsafe { libc::fcntl(inherited.as_raw_fd(), libc::F_SETFD, 0) },
        -1
    );
    let args = [
        "-c".into(),
        script.into(),
        allowed.as_os_str().to_owned(),
        denied.as_os_str().to_owned(),
        inherited.as_raw_fd().to_string().into(),
    ];
    let mut launch =
        sandbox::command("/usr/bin/python3", args, &policy(allowed.clone(), true)).unwrap();
    launch
        .env(super::super::HELPER_BACKEND_ENV, "bwrap")
        .env("ANGEL_YOLO", "0");
    let output = launch.output().unwrap();
    drop(inherited);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("nested-and-confinement-ok"));
    assert!(!denied.join("outside").exists());

    let file_root = denied.join("approved-file");
    std::fs::write(&file_root, "before").unwrap();
    let mut file_policy = policy(allowed.clone(), true);
    file_policy.writable_roots.push(file_root.clone());
    let mut launch = sandbox::command(
        "/usr/bin/python3",
        [
            "-c",
            "import pathlib,sys; pathlib.Path(sys.argv[1]).write_text('after')",
            file_root.to_str().unwrap(),
        ],
        &file_policy,
    )
    .unwrap();
    launch
        .env(super::super::HELPER_BACKEND_ENV, "bwrap")
        .env("ANGEL_YOLO", "0");
    let output = launch.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_to_string(&file_root).unwrap(), "after");

    let network_script = r#"
import socket
for family in (socket.AF_INET, socket.AF_INET6):
 try: socket.socket(family, socket.SOCK_STREAM)
 except PermissionError: pass
 else: raise AssertionError('Internet socket creation must be denied')
left,right=socket.socketpair()
left.close(); right.close()
print('network-denied-unix-retained')
"#;
    let mut launch = sandbox::command(
        "/usr/bin/python3",
        ["-c", network_script],
        &policy(allowed.clone(), false),
    )
    .unwrap();
    launch
        .env(super::super::HELPER_BACKEND_ENV, "bwrap")
        .env("ANGEL_YOLO", "0");
    let output = launch.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("network-denied-unix-retained"));

    let mut launch = sandbox::command(
        "/usr/bin/python3",
        [
            "-c",
            "import pathlib,sys; pathlib.Path(sys.argv[1]).write_text('bad')",
            allowed.join("reviewer-must-not-write").to_str().unwrap(),
        ],
        &SandboxPolicy::read_only(),
    )
    .unwrap();
    launch
        .env(super::super::HELPER_BACKEND_ENV, "bwrap")
        .env("ANGEL_YOLO", "1");
    let output = launch.output().unwrap();
    assert!(
        !output.status.success(),
        "reviewer ceiling must survive backend and YOLO selection"
    );
    assert!(!allowed.join("reviewer-must-not-write").exists());

    // The parent launcher exits after the managed command reaches readiness.
    // A delayed write then proves that detached jobs survive parent exit;
    // killing their owned process group must still reap every descendant.
    let detached_script = r#"
import pathlib,sys,time
base=pathlib.Path(sys.argv[1])
(base/'detached-ready').write_text('ready')
time.sleep(1)
(base/'detached-survived').write_text('yes')
time.sleep(30)
"#;
    let mut detached = sandbox::command(
        "/usr/bin/python3",
        ["-c", detached_script, allowed.to_str().unwrap()],
        &policy(allowed.clone(), true),
    )
    .unwrap();
    sandbox::set_detached_lifecycle(&mut detached);
    detached
        .env(super::super::HELPER_BACKEND_ENV, "bwrap")
        .env("ANGEL_YOLO", "0");
    let spec = serde_json::json!({
        "argv": std::iter::once(detached.get_program().to_string_lossy().into_owned()).chain(detached.get_args().map(|arg| arg.to_string_lossy().into_owned())).collect::<Vec<_>>(),
        "env": detached.get_envs().filter_map(|(key,value)| value.map(|value| (key.to_string_lossy().into_owned(), value.to_string_lossy().into_owned()))).collect::<std::collections::BTreeMap<_,_>>(),
        "ready": allowed.join("detached-ready"),
    });
    let launcher_script = r#"
import json,os,pathlib,subprocess,sys,time
spec=json.loads(sys.argv[1]); env=os.environ.copy(); env.update(spec['env'])
child=subprocess.Popen(spec['argv'],env=env,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)
print(child.pid,flush=True)
for _ in range(100):
 if pathlib.Path(spec['ready']).exists(): sys.exit(0)
 if child.poll() is not None: sys.exit(2)
 time.sleep(.05)
child.kill(); child.wait(); sys.exit(3)
"#;
    let launched = Command::new("/usr/bin/python3")
        .args(["-c", launcher_script, &spec.to_string()])
        .output()
        .unwrap();
    assert!(
        launched.status.success(),
        "detached launcher: {}",
        String::from_utf8_lossy(&launched.stderr)
    );
    let pid: i32 = String::from_utf8_lossy(&launched.stdout)
        .trim()
        .parse()
        .unwrap();
    struct ProcessGroup(i32);
    impl Drop for ProcessGroup {
        fn drop(&mut self) {
            unsafe {
                libc::kill(-self.0, libc::SIGKILL);
            }
        }
    }
    let group = ProcessGroup(pid);
    for _ in 0..100 {
        if allowed.join("detached-survived").exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    assert!(
        allowed.join("detached-survived").exists(),
        "detached verifier must outlive its launcher"
    );
    let mut descendants = vec![pid];
    let mut cursor = 0;
    while cursor < descendants.len() && descendants.len() < 16 {
        let p = descendants[cursor];
        cursor += 1;
        let children =
            std::fs::read_to_string(format!("/proc/{p}/task/{p}/children")).unwrap_or_default();
        descendants.extend(
            children
                .split_whitespace()
                .filter_map(|id| id.parse::<i32>().ok()),
        );
    }
    assert!(
        descendants.len() > 1,
        "must observe the managed namespace descendants"
    );
    drop(group);
    for _ in 0..100 {
        let alive = descendants.iter().any(|pid| {
            std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
                !stat
                    .rsplit_once(") ")
                    .is_some_and(|(_, tail)| tail.starts_with('Z'))
            })
        });
        if !alive {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    for pid in descendants {
        assert!(
            std::fs::read_to_string(format!("/proc/{pid}/stat")).map_or(true, |stat| stat
                .rsplit_once(") ")
                .is_some_and(|(_, tail)| tail.starts_with('Z'))),
            "detached process group left a live namespace descendant {pid}"
        );
    }
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn sealed_mount_plan_has_no_host_root_and_forces_private_namespaces() {
    let _guard = crate::tests::env_lock();
    let base = std::env::temp_dir().join(format!("sealed-bwrap-plan-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    let profile = sandbox::sealed::build(&base, None);
    let prepared = command(
        &std::env::current_exe().unwrap(),
        &profile.policy,
        "true".into(),
        Vec::new(),
        false,
    )
    .unwrap();
    let args: Vec<_> = prepared
        .command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert!(!args.windows(3).any(|a| a == ["--ro-bind", "/", "/"]));
    for required in ["--unshare-net", "--unshare-pid", "--ro-bind-fd", "--proc"] {
        assert!(args.iter().any(|arg| arg == required), "missing {required}");
    }
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn hardlink_overlays_follow_writable_mounts() {
    let _lock = crate::tests::env_lock();
    let fixture = super::super::HardlinkTestRoot::new();
    let root = fixture.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    let outside = fixture.path().join("outside");
    std::fs::write(&outside, "before").unwrap();
    let alias = root.join("alias");
    std::fs::hard_link(&outside, &alias).unwrap();
    for policy in [
        policy(root.clone(), true),
        sandbox::sealed::build(&root, None).policy,
    ] {
        let prepared = command(
            &std::env::current_exe().unwrap(),
            &policy,
            "true".into(),
            Vec::new(),
            false,
        )
        .unwrap();
        let args: Vec<_> = prepared.command.get_args().collect();
        let writable = args.iter().rposition(|arg| *arg == "--bind-fd").unwrap();
        let readonly = args.iter().rposition(|arg| *arg == "--ro-bind-fd").unwrap();
        assert!(readonly > writable);
        assert_eq!(args[readonly + 2], alias.as_os_str());
    }
}

#[test]
fn large_hardlinked_build_tree_still_executes_with_confinement() {
    let _lock = crate::tests::env_lock();
    let probe = Command::new(EXECUTABLE)
        .args(["--unshare-user", "--ro-bind", "/", "/", "--", "/bin/true"])
        .output();
    if !probe.as_ref().is_ok_and(|output| output.status.success()) {
        assert_ne!(std::env::var("ANGEL_BWRAP_STRICT").as_deref(), Ok("1"));
        eprintln!("large-workspace check requires host Bubblewrap namespaces");
        return;
    }
    let fixture = super::super::HardlinkTestRoot::new();
    let root = fixture.path().join("workspace");
    let target = root.join("target");
    std::fs::create_dir_all(&target).unwrap();
    let outside = fixture.path().join("outside");
    std::fs::write(&outside, "unchanged").unwrap();
    for index in 0..8200 {
        std::fs::hard_link(&outside, target.join(format!("artifact-{index:05}.o"))).unwrap();
    }
    for index in 0..2200 {
        std::fs::write(root.join(format!("source-{index:05}")), "source").unwrap();
    }
    let preload_source = root.join("preload.c");
    let preload = root.join("preload.so");
    std::fs::write(
        &preload_source,
        r#"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
__attribute__((constructor)) static void inspect_setup_privileges(void) {
    FILE *status = fopen("/proc/self/status", "r");
    if (!status) _exit(90);
    char line[256];
    while (fgets(line, sizeof(line), status)) {
        unsigned long long bits;
        if (sscanf(line, "CapEff: %llx", &bits) == 1 && bits) _exit(91);
    }
    fclose(status);
    const char *path = getenv("ANGEL_TEST_PRELOAD_MARKER");
    FILE *marker = path ? fopen(path, "a") : NULL;
    if (marker) { fputs("no-setup-privileges\n", marker); fclose(marker); }
}
"#,
    )
    .unwrap();
    let compiled = Command::new("cc")
        .args(["-shared", "-fPIC"])
        .arg(&preload_source)
        .arg("-o")
        .arg(&preload)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let script = r#"
import errno, os, pathlib, resource, socket, subprocess, sys
root = pathlib.Path(sys.argv[1])
status = dict(line.split(':',1) for line in pathlib.Path('/proc/self/status').read_text().splitlines() if ':' in line)
for key in ('CapEff','CapPrm','CapInh','CapBnd','CapAmb'): assert int(status[key],16) == 0, (key,status[key])
assert int(status['NoNewPrivs']) == 1
assert resource.getrlimit(resource.RLIMIT_NOFILE)[0] == 1024
assert os.environ['ANGEL_TEST_ROUNDTRIP'] == 'spaces, quotes: " and Unicode λ'
assert os.environ['LD_PRELOAD'].endswith('/preload.so')
if os.environ['ANGEL_TEST_NETWORK_ALLOWED'] == 'false':
 try: socket.socket(socket.AF_INET, socket.SOCK_STREAM)
 except OSError as error: assert error.errno in (errno.EPERM, errno.EACCES), error
 else: raise AssertionError('network-disabled worker created an Internet socket')
# Exercise every alias, including every setup-stage boundary.
for alias in (root/'target').glob('artifact-*'):
 try: alias.write_text('escaped')
 except OSError as error: assert error.errno == errno.EROFS, (alias, error)
 else: raise AssertionError('external hardlink was writable: '+str(alias))
for name in os.listdir('/proc/self/fd'):
 if int(name) <= 2: continue
 try: os.fstat(int(name))
 except OSError: continue
 raise AssertionError('setup descriptor leaked to payload: '+name)
try: pathlib.Path('/outside').write_text('escaped')
except OSError: pass
else: raise AssertionError('outside workspace was writable')
if (root.parent/'outside').exists():
 try: (root.parent/'outside').write_text('escaped')
 except OSError: pass
 else: raise AssertionError('host outside file was writable')
(root/'target'/'new-result').write_text('iteration completed')
subprocess.run(['/bin/sh','-c','printf child-ok > child-result'], cwd=root, check=True)
assert (root/'child-result').read_text() == 'child-ok'
print('large-workspace-write-and-confinement-ok')
"#;
    // Leave backend selection at its ordinary default: discovering external
    // aliases must select and successfully execute the protected path itself.
    for profile in [
        policy(root.clone(), true),
        policy(root.clone(), false),
        sandbox::sealed::build(&root, None).policy,
    ] {
        let mut worker = sandbox::command(
            "/usr/bin/python3",
            ["-c", script, root.to_str().unwrap()],
            &profile,
        )
        .unwrap();
        worker
            .env(
                super::super::HELPER_BACKEND_ENV,
                if profile.sealed_reads.is_empty() {
                    "landlock"
                } else {
                    "bwrap"
                },
            )
            .env("ANGEL_YOLO", "0")
            .env(
                "ANGEL_TEST_NETWORK_ALLOWED",
                profile.allow_network.to_string(),
            )
            .env("ANGEL_TEST_ROUNDTRIP", "spaces, quotes: \" and Unicode λ")
            .env("ANGEL_TEST_PRELOAD_MARKER", root.join("preload-runs"))
            .env("LD_PRELOAD", &preload);
        // Mirror SSH's 1,024 soft limit on Apollo/Spark without modifying the
        // multithreaded test runner. Only async-signal-safe libc calls here.
        unsafe {
            worker.pre_exec(|| {
                let mut limit = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) != 0 {
                    return Err(io::Error::last_os_error());
                }
                limit.rlim_cur = 1024;
                if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let output = worker.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains("large-workspace-write-and-confinement-ok")
        );
    }
    assert!(
        std::fs::read_to_string(root.join("preload-runs"))
            .unwrap()
            .lines()
            .count()
            >= 6
    );
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "unchanged");
    assert_eq!(
        std::fs::read_to_string(target.join("new-result")).unwrap(),
        "iteration completed"
    );
}

#[test]
#[ignore = "needs-host-feature: bwrap"]
fn bwrap_hardlink_write_is_erofs_and_new_local_links_work() {
    let _lock = crate::tests::env_lock();
    let fixture = super::super::HardlinkTestRoot::new();
    let root = fixture.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    let outside = fixture.path().join("outside");
    std::fs::write(&outside, "before").unwrap();
    std::fs::hard_link(&outside, root.join("alias")).unwrap();
    let script = r#"
import errno, os, pathlib, sys
root = pathlib.Path(sys.argv[1])
try:
    (root/'alias').write_text('after')
except OSError as error:
    assert error.errno == errno.EROFS, error
else:
    raise AssertionError('hard link was writable')
(root/'local').write_text('local')
os.link(root/'local', root/'local_alias')
(root/'local_alias').write_text('new local')
assert (root/'local').read_text() == 'new local'
"#;
    let output = command(
        Path::new(EXECUTABLE),
        &policy(root.clone(), true),
        "/usr/bin/python3".into(),
        ["-c".into(), script.into(), root.into_os_string()],
        false,
    )
    .unwrap()
    .command
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "before");
}

#[test]
fn sealed_posture_mounts_a_private_writable_tmp() {
    let _lock = crate::tests::env_lock();
    let fixture = super::super::HardlinkTestRoot::new();
    let root = fixture.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    let plan = |policy: &SandboxPolicy| -> Vec<String> {
        command(
            &std::env::current_exe().unwrap(),
            policy,
            "true".into(),
            Vec::new(),
            false,
        )
        .unwrap()
        .command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
    };
    let tmp_mount = ["--perms", "01777", "--tmpfs", "/tmp"];

    let sealed = plan(&sandbox::sealed::build(&root, None).policy);
    let tmp = sealed
        .windows(4)
        .position(|a| a == tmp_mount)
        .expect("the sealed root is a bare tmpfs and must still offer a writable /tmp");
    // A policy that explicitly grants /tmp must bind host /tmp over ours, so
    // the private mount has to come first.
    let writable = sealed.iter().position(|arg| arg == "--bind-fd").unwrap();
    assert!(tmp < writable, "private /tmp must precede writable roots");
    // `--remount-ro /` seals the root afterwards; /tmp is a separate mount and
    // the remount is not recursive, so it stays writable.
    let seal = sealed.iter().position(|arg| arg == "--remount-ro").unwrap();
    assert!(tmp < seal, "private /tmp must be mounted before the seal");

    // `--ro-bind / /` promises read-all: a private /tmp there would hide the
    // host's from the payload, so that posture must not gain this mount.
    let permissive = plan(&policy(root, true));
    assert!(
        !permissive.windows(4).any(|a| a == tmp_mount),
        "read-all posture must keep host /tmp visible"
    );
}

#[test]
#[ignore = "needs-host-feature: bwrap"]
fn sealed_tmp_is_writable_from_a_read_only_cwd_and_is_not_host_tmp() {
    let _lock = crate::tests::env_lock();
    let fixture = super::super::HardlinkTestRoot::new();
    let root = fixture.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    let marker = std::env::temp_dir().join(format!("angel-host-tmp-{}", std::process::id()));
    std::fs::write(&marker, "host").unwrap();
    // `cd /` puts the payload on a read-only cwd: the shape that made gcc
    // abort once libiberty fell back to "./" for its resolution file.
    let script = format!(
        "set -e; cd /; : > /tmp/probe; test ! -e {}; echo sealed-tmp-ok",
        marker.display()
    );
    let mut launch = sandbox::command(
        "/bin/sh",
        ["-c".into(), std::ffi::OsString::from(script)],
        &sandbox::sealed::build(&root, None).policy,
    )
    .unwrap();
    launch
        .env(super::super::HELPER_BACKEND_ENV, "bwrap")
        .env("ANGEL_YOLO", "0");
    let output = launch.output().unwrap();
    std::fs::remove_file(&marker).ok();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("sealed-tmp-ok"));
}
