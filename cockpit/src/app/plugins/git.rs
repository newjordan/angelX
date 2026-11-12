//! Fetch one folder of a repository at one commit, through the `git` program.
//!
//! The fetch is shallow and partial: the commit and its trees first, then the
//! blobs under the plugin's folder and nothing else, so a plugin kept inside a
//! large repository costs its own size. A server without partial clone sends
//! the whole commit instead; the result is the same. Files are read from git
//! objects, never from a checkout, so `.gitattributes` filters cannot make
//! the bytes differ from what the digest names.

use super::digest::{Entry, Kind};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The program, `ANGEL_PLUGIN_GIT` or `git`.
fn program() -> String {
    std::env::var("ANGEL_PLUGIN_GIT")
        .ok()
        .filter(|git| !git.trim().is_empty())
        .unwrap_or_else(|| "git".to_owned())
}

/// A 40-character lowercase hex commit id.
pub(crate) fn valid_commit(commit: &str) -> bool {
    commit.len() == 40
        && commit
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// A repository address an install may fetch from: https (what the public
/// registry takes), http, a `file://` URL or an absolute path (private
/// registries and tests).
pub(crate) fn fetchable(url: &str) -> bool {
    let tail = |scheme: &str| {
        url.strip_prefix(scheme)
            .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('-'))
    };
    (tail("https://")
        || tail("http://")
        || tail("file://")
        || (url.starts_with('/') && url.len() > 1))
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Run git in `dir` and return its stdout; a failure carries the end of stderr.
pub(crate) fn run(
    dir: &Path,
    args: &[&str],
    stdin: Option<&[u8]>,
    cancel: Option<&AtomicBool>,
) -> Result<Vec<u8>, String> {
    let mut command = Command::new(program());
    command
        .args(args)
        .current_dir(dir)
        // A private repository must fail, not wait for a password.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("SSH_ASKPASS", "")
        .env("LC_ALL", "C")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command
        .spawn()
        .map_err(|e| format!("git: {e} (installing plugins needs git on PATH)"))?;
    let feed = stdin.map(<[u8]>::to_vec);
    let mut pipe_in = child.stdin.take();
    let writer = std::thread::spawn(move || {
        if let (Some(pipe), Some(bytes)) = (pipe_in.as_mut(), feed) {
            let _ = pipe.write_all(&bytes);
        }
    });
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let status = loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::Acquire)) {
            kill(&mut child);
            return Err("cancelled".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(15)),
            Err(e) => return Err(format!("git: {e}")),
        }
    };
    let _ = writer.join();
    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    if status.success() {
        return Ok(stdout);
    }
    let text = String::from_utf8_lossy(&stderr);
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let tail = lines[lines.len().saturating_sub(3)..].join(" · ");
    Err(format!(
        "git {} failed: {}",
        args.first().copied().unwrap_or(""),
        if tail.is_empty() {
            status.to_string()
        } else {
            tail
        }
    ))
}

/// Stop a child and everything it started.
pub(crate) fn kill(child: &mut std::process::Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    })
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).trim().to_owned()
}

/// The files of `path` (the repository root when empty) at `commit` of the
/// repository at `url`, fetched into a fresh bare repository at `scratch`.
pub(crate) fn fetch(
    url: &str,
    commit: &str,
    path: &str,
    scratch: &Path,
    cancel: Option<&AtomicBool>,
) -> Result<Vec<Entry>, String> {
    if !valid_commit(commit) {
        return Err(format!("`{commit}` is not a full commit id"));
    }
    if !fetchable(url) {
        return Err(format!(
            "`{url}` is not an https, http or file URL, or an absolute path"
        ));
    }
    std::fs::create_dir_all(scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    run(scratch, &["init", "-q", "--bare", "."], None, cancel)?;
    for setting in [
        ["remote.origin.url", url],
        ["remote.origin.promisor", "true"],
        ["remote.origin.partialclonefilter", "blob:none"],
    ] {
        run(scratch, &["config", setting[0], setting[1]], None, cancel)?;
    }
    let shallow = run(
        scratch,
        &[
            "fetch",
            "-q",
            "--no-tags",
            "--depth",
            "1",
            "--filter=blob:none",
            "origin",
            commit,
        ],
        None,
        cancel,
    );
    if let Err(first) = shallow {
        if first == "cancelled" {
            return Err(first);
        }
        // A server that will not send an unadvertised commit by id: take its
        // branches and look for the commit among them.
        run(
            scratch,
            &[
                "fetch",
                "-q",
                "--no-tags",
                "--filter=blob:none",
                "origin",
                "+refs/heads/*:refs/remotes/origin/*",
            ],
            None,
            cancel,
        )
        .map_err(|second| format!("cannot fetch {url} at {commit}: {first}; {second}"))?;
    }
    let found = run(
        scratch,
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
        None,
        cancel,
    )
    .map(text)
    .map_err(|_| format!("{url} has no commit {commit}"))?;
    if found != commit {
        return Err(format!("{url}: {commit} resolves to {found}"));
    }
    let tree = if path.is_empty() {
        format!("{commit}^{{tree}}")
    } else {
        format!("{commit}:{path}")
    };
    let listing = run(scratch, &["ls-tree", "-r", "-z", &tree], None, cancel)
        .map_err(|_| format!("{url} at {commit} has no folder `{path}`"))?;
    let mut files: Vec<(String, Kind, String)> = Vec::new();
    for record in listing
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let record =
            std::str::from_utf8(record).map_err(|_| "a file name that is not UTF-8".to_string())?;
        let (meta, name) = record
            .split_once('\t')
            .ok_or_else(|| format!("unreadable ls-tree record `{record}`"))?;
        let mut fields = meta.split(' ');
        let (mode, kind, oid) = (
            fields.next().unwrap_or(""),
            fields.next().unwrap_or(""),
            fields.next().unwrap_or(""),
        );
        if !super::digest::safe_path(name) {
            return Err(format!("`{name}` is not a path a plugin may carry"));
        }
        let kind = match (mode, kind) {
            ("100755", "blob") => Kind::Exec,
            ("120000", "blob") => Kind::Link,
            (mode, "blob") if mode.starts_with("100") => Kind::File,
            ("160000", _) => {
                return Err(format!(
                    "`{name}` is a git submodule; a plugin carries its own files"
                ));
            }
            _ => return Err(format!("`{name}` has git mode {mode} {kind}")),
        };
        files.push((name.to_owned(), kind, oid.to_owned()));
    }
    if files.is_empty() {
        return Err(format!("{url} at {commit} has nothing in `{path}`"));
    }
    let oids: Vec<&str> = files.iter().map(|(_, _, oid)| oid.as_str()).collect();
    fetch_missing(scratch, &oids, cancel)?;
    let blobs = read_blobs(scratch, &oids, cancel)?;
    Ok(files
        .into_iter()
        .zip(blobs)
        .map(|((path, kind, _), bytes)| Entry { path, kind, bytes })
        .collect())
}

/// Fetch the blobs a partial fetch left out, in batches.
fn fetch_missing(scratch: &Path, oids: &[&str], cancel: Option<&AtomicBool>) -> Result<(), String> {
    let query = oids
        .iter()
        .map(|oid| format!("{oid}\n"))
        .collect::<String>();
    let mut command = Command::new(program());
    command
        .args(["cat-file", "--batch-check"])
        .current_dir(scratch)
        // Without this an older git fetches each missing blob on its own,
        // which still works, only slower.
        .env("GIT_NO_LAZY_FETCH", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(|e| format!("git: {e}"))?;
    let mut input = child.stdin.take();
    let writer = std::thread::spawn(move || {
        if let Some(pipe) = input.as_mut() {
            let _ = pipe.write_all(query.as_bytes());
        }
    });
    let output = child.wait_with_output().map_err(|e| format!("git: {e}"))?;
    let _ = writer.join();
    let missing: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.ends_with(" missing"))
        .filter_map(|line| line.split(' ').next().map(str::to_owned))
        .collect();
    for batch in missing.chunks(256) {
        let mut args = vec![
            "-c",
            "fetch.negotiationAlgorithm=noop",
            "fetch",
            "-q",
            "--no-tags",
            "--no-write-fetch-head",
            "--filter=blob:none",
            "origin",
        ];
        args.extend(batch.iter().map(String::as_str));
        run(scratch, &args, None, cancel)?;
    }
    Ok(())
}

/// The contents of `oids`, in order.
fn read_blobs(
    scratch: &Path,
    oids: &[&str],
    cancel: Option<&AtomicBool>,
) -> Result<Vec<Vec<u8>>, String> {
    let query = oids
        .iter()
        .map(|oid| format!("{oid}\n"))
        .collect::<String>();
    let output = run(
        scratch,
        &["cat-file", "--batch"],
        Some(query.as_bytes()),
        cancel,
    )?;
    let mut blobs = Vec::with_capacity(oids.len());
    let mut at = 0usize;
    for oid in oids {
        let end = output[at..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| at + offset)
            .ok_or_else(|| format!("git cat-file ended before {oid}"))?;
        let header = std::str::from_utf8(&output[at..end]).unwrap_or("");
        let mut fields = header.split(' ');
        let (got, kind, size) = (
            fields.next().unwrap_or(""),
            fields.next().unwrap_or(""),
            fields.next().unwrap_or(""),
        );
        if got != *oid || kind != "blob" {
            return Err(format!("git could not read {oid}: {header}"));
        }
        let size: usize = size
            .parse()
            .map_err(|_| format!("git cat-file: bad size in `{header}`"))?;
        let start = end + 1;
        let stop = start + size;
        if output.len() < stop + 1 {
            return Err(format!("git cat-file: {oid} is cut short"));
        }
        blobs.push(output[start..stop].to_vec());
        at = stop + 1;
    }
    Ok(blobs)
}
