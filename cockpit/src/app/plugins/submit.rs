//! `angelX plugin submit`: from a pushed plugin to a registry pull request.
//!
//! The plugin is fetched back from its public repository at the commit being
//! submitted, exactly as an install will fetch it, and checked to the
//! registry's level. Its entry is computed from what came back, never from
//! the working folder. Then, through the GitHub CLI and as its signed-in
//! user, the entry goes onto a branch of their fork of the angelX repository
//! and a pull request opens; or the entry is printed, or written into a
//! registry file, for whoever opens the request another way.

use super::check::{self, Level};
use super::install::Staging;
use super::registry::{self, Change, Entry};
use super::{Paths, REGISTRY_FILE, UPSTREAM, digest, git};
use crate::agent::harness::cartridges::config::Config;
use base64::Engine as _;
use std::path::Path;
use std::sync::atomic::AtomicBool;

/// A plugin ready to submit.
pub(crate) struct Prepared {
    pub(crate) entry: Entry,
    pub(crate) config: Config,
    pub(crate) report: check::Report,
}

/// Fetch the plugin at `dir` back from its repository and build its entry.
/// `commit` defaults to HEAD, which must be committed and pushed;
/// `repository` defaults to `[plugin] repository`, then the git remote.
/// `public` holds the entry to the public registry's rules (an https
/// repository); a private registry may take any address git can fetch.
pub(crate) fn prepare(
    paths: &Paths,
    dir: &Path,
    commit: Option<&str>,
    repository: Option<&str>,
    public: bool,
    cancel: Option<&AtomicBool>,
) -> Result<Prepared, String> {
    let dir = dir
        .canonicalize()
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    let output = |args: &[&str]| {
        git::run(&dir, args, None, cancel)
            .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
    };
    let root = output(&["rev-parse", "--show-toplevel"]).map_err(|_| {
        format!(
            "{} is not in a git repository; a plugin is published from one",
            dir.display()
        )
    })?;
    let root = Path::new(&root)
        .canonicalize()
        .map_err(|e| format!("{root}: {e}"))?;
    let rel = dir
        .strip_prefix(&root)
        .map_err(|_| {
            format!(
                "{} is outside its repository {}",
                dir.display(),
                root.display()
            )
        })?
        .to_string_lossy()
        .replace('\\', "/");
    if !rel.is_empty() && !digest::safe_path(&rel) {
        return Err(format!("`{rel}` is not a folder the registry can name"));
    }
    let local = check::folder(&dir, Level::Load);
    let local_repository = local
        .config
        .as_ref()
        .and_then(|config| config.plugin.as_ref())
        .and_then(|plugin| plugin.repository.clone());
    let commit = match commit {
        Some(given)
            if given.starts_with('-')
                || !given
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "/._-^~".contains(c)) =>
        {
            return Err(format!("`{given}` is not a commit"));
        }
        Some(given) => match output(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{given}^{{commit}}"),
        ]) {
            Ok(full) if git::valid_commit(&full) => full,
            _ if git::valid_commit(given) => given.to_owned(),
            _ => {
                return Err(format!(
                    "`{given}` is not a commit here; give the full 40-character id"
                ));
            }
        },
        None => {
            let dirty = output(&["status", "--porcelain", "--untracked-files=all", "--", "."])?;
            if !dirty.is_empty() {
                let files: Vec<&str> = dirty.lines().take(5).collect();
                return Err(format!(
                    "the plugin has changes that are not committed; commit and push them first:\n  {}",
                    files.join("\n  ")
                ));
            }
            output(&["rev-parse", "HEAD"])?
        }
    };
    let repository = match repository.map(str::to_owned).or(local_repository) {
        Some(url) => url,
        None => {
            let remote = output(&[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ])
            .ok()
            .and_then(|upstream| upstream.split('/').next().map(str::to_owned))
            .unwrap_or_else(|| "origin".to_owned());
            let url = output(&["remote", "get-url", &remote]).map_err(|_| {
                "no git remote to publish from: push the plugin to a public repository first"
                    .to_string()
            })?;
            public_url(&url)
        }
    };
    let staging = Staging::new(paths, "submit")?;
    let files = git::fetch(&repository, &commit, &rel, &staging.path().join("repo.git"), cancel).map_err(|error| {
        format!("cannot fetch {} from {repository}: {error}\n  (push the commit, and check the repository is public)", install_short(&commit))
    })?;
    let folder = staging.path().join("plugin");
    digest::write_folder(&folder, &files)?;
    let report = check::folder(&folder, Level::Publish);
    if !report.ok() {
        return Err(format!(
            "the plugin at {} is not ready for the registry:\n{}",
            install_short(&commit),
            report.lines().join("\n")
        ));
    }
    let config = report
        .config
        .clone()
        .expect("a passing check parsed the cartridge");
    let plugin = config
        .plugin
        .clone()
        .expect("a publish check requires [plugin]");
    let entry = Entry {
        id: config.id.clone(),
        label: config.label.clone(),
        version: plugin.version,
        description: plugin.description,
        repository,
        commit,
        path: rel,
        sha256: digest::digest(&files),
        license: plugin.license,
        authors: plugin.authors,
        keywords: plugin.keywords,
        homepage: plugin.homepage,
    };
    let problems = entry.problems(public);
    if !problems.is_empty() {
        return Err(format!(
            "the registry entry would be refused:\n  ✗ {}",
            problems.join("\n  ✗ ")
        ));
    }
    Ok(Prepared {
        entry,
        config,
        report,
    })
}

fn install_short(commit: &str) -> &str {
    super::install::short(commit)
}

/// An https address for a git remote: `git@host:owner/repo.git` and
/// `ssh://git@host/owner/repo` become `https://host/owner/repo`; credentials
/// and a trailing `.git` are dropped.
pub(crate) fn public_url(remote: &str) -> String {
    let remote = remote.trim();
    let (host, path) = if let Some(rest) = remote.strip_prefix("ssh://") {
        let rest = rest.rsplit_once('@').map_or(rest, |(_, after)| after);
        match rest.split_once('/') {
            Some((host, path)) => (
                host.split(':').next().unwrap_or(host).to_owned(),
                path.to_owned(),
            ),
            None => return remote.to_owned(),
        }
    } else if let Some(rest) = remote
        .strip_prefix("https://")
        .or_else(|| remote.strip_prefix("http://"))
    {
        let rest = match rest.split_once('/') {
            Some((authority, path)) => (
                authority
                    .rsplit_once('@')
                    .map_or(authority, |(_, host)| host)
                    .to_owned(),
                path.to_owned(),
            ),
            None => return remote.to_owned(),
        };
        rest
    } else if let Some((user_host, path)) = remote.split_once(':')
        && !user_host.contains('/')
        && user_host.contains('@')
    {
        (
            user_host
                .rsplit_once('@')
                .map_or(user_host, |(_, host)| host)
                .to_owned(),
            path.to_owned(),
        )
    } else {
        return remote.to_owned();
    };
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    format!("https://{host}/{path}")
}

/// What a plugin brings, in one line: its commands, tools, pages and setup.
pub(crate) fn carries(config: &Config) -> String {
    let mut parts: Vec<String> = config
        .commands
        .iter()
        .map(|command| format!("/{}", command.name))
        .collect();
    parts.extend(config.tools.iter().map(|tool| {
        format!(
            "{}{}",
            tool.name,
            if tool.core { " (core tool)" } else { " (tool)" }
        )
    }));
    if !config.pages.is_empty() {
        parts.push(format!("{} book page set(s)", config.pages.len()));
    }
    if !config.boards.is_empty() || !config.worker.is_empty() {
        parts.push("a competition".into());
    }
    if let Some(setup) = config
        .plugin
        .as_ref()
        .and_then(|plugin| plugin.setup.as_deref())
    {
        parts.push(format!("setup {setup}"));
    }
    if parts.is_empty() {
        "nothing".into()
    } else {
        parts.join(" · ")
    }
}

/// The GitHub CLI: `ANGEL_PLUGIN_GH`, else `gh`.
fn gh(args: &[&str], stdin: Option<&[u8]>) -> Result<String, String> {
    let program = std::env::var("ANGEL_PLUGIN_GH")
        .ok()
        .filter(|gh| !gh.trim().is_empty())
        .unwrap_or_else(|| "gh".to_owned());
    use std::io::Write;
    let mut child = std::process::Command::new(&program)
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(if stdin.is_some() { std::process::Stdio::piped() } else { std::process::Stdio::null() })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("{program}: {e} — the GitHub CLI (https://cli.github.com) opens the pull request; or `angelX plugin submit --print`"))?;
    if let (Some(mut pipe), Some(bytes)) = (child.stdin.take(), stdin) {
        let _ = pipe.write_all(bytes);
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("{program}: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim().to_owned()
        } else {
            stderr.trim().to_owned()
        };
        Err(format!(
            "gh {}: {}",
            args.iter().take(2).copied().collect::<Vec<_>>().join(" "),
            detail.lines().last().unwrap_or("failed")
        ))
    }
}

/// The commit message and pull request title for a change.
fn title(entry: &Entry, change: &Change) -> String {
    match change {
        Change::Added => format!("plugins: add {} {}", entry.id, entry.version),
        Change::Updated(from) if from == &entry.version => format!(
            "plugins: {} {} at {}",
            entry.id,
            entry.version,
            super::install::short(&entry.commit)
        ),
        Change::Updated(from) => format!("plugins: {} {from} → {}", entry.id, entry.version),
        Change::Unchanged => format!("plugins: {} {}", entry.id, entry.version),
    }
}

fn body(prepared: &Prepared, change: &Change) -> String {
    let entry = &prepared.entry;
    let what = match change {
        Change::Added => "Adds",
        _ => "Updates",
    };
    format!(
        "{what} the angelX plugin **{}** (`{}` {}) in the registry.\n\n> {}\n\n\
         - Source: {} at `{}`{}\n- Tree digest: `{}`\n- License: {} · Authors: {}\n- Carries: {}\n\n\
         Opened by `angelX plugin submit`. Review: `scripts/plugins/check-registry.py --changed origin/main` \
         fetches that commit, recomputes the digest and checks the plugin; then a maintainer reads its code.\n",
        entry.label,
        entry.id,
        entry.version,
        entry.description,
        entry.repository,
        entry.commit,
        if entry.path.is_empty() {
            String::new()
        } else {
            format!(", folder `{}`", entry.path)
        },
        entry.sha256,
        entry.license,
        entry.authors.join(", "),
        carries(&prepared.config),
    )
}

/// Write the entry into a local registry file.
pub(crate) fn into_file(prepared: &Prepared, file: &Path) -> Result<String, String> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("{}: {error}", file.display())),
    };
    let (next, change) = registry::upsert(&text, &prepared.entry)?;
    if change == Change::Unchanged {
        return Ok(format!(
            "{} already lists {} {} at that commit",
            file.display(),
            prepared.entry.id,
            prepared.entry.version
        ));
    }
    std::fs::write(file, next).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(format!(
        "{} · {}",
        title(&prepared.entry, &change),
        file.display()
    ))
}

/// Open the registry pull request on the angelX repository as the GitHub
/// CLI's signed-in user. Returns the pull request's address.
pub(crate) fn open_pull_request(prepared: &Prepared) -> Result<String, String> {
    let entry = &prepared.entry;
    gh(&["auth", "status"], None).map_err(|_| {
        "the GitHub CLI is not signed in: `gh auth login`, then submit again (or `angelX plugin submit --print` for the entry)".to_string()
    })?;
    let login = gh(&["api", "user", "--jq", ".login"], None)?;
    let (owner, name) = UPSTREAM.split_once('/').expect("owner/name");
    let base = gh(
        &[
            "api",
            &format!("repos/{UPSTREAM}/git/ref/heads/main"),
            "--jq",
            ".object.sha",
        ],
        None,
    )?;
    let file: serde_json::Value = serde_json::from_str(&gh(
        &[
            "api",
            &format!("repos/{UPSTREAM}/contents/{REGISTRY_FILE}?ref={base}"),
        ],
        None,
    )?)
    .map_err(|e| format!("reading {REGISTRY_FILE} from {UPSTREAM}: {e}"))?;
    let blob = file["sha"]
        .as_str()
        .ok_or("the registry file has no sha")?
        .to_owned();
    let encoded: String = file["content"]
        .as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let text = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| format!("decoding {REGISTRY_FILE}: {e}"))?,
    )
    .map_err(|e| format!("{REGISTRY_FILE}: {e}"))?;
    let (next, change) = registry::upsert(&text, entry)?;
    if change == Change::Unchanged {
        return Err(format!(
            "the registry already lists {} {} at that commit",
            entry.id, entry.version
        ));
    }
    let head_repo = if login.eq_ignore_ascii_case(owner) {
        UPSTREAM.to_owned()
    } else {
        let _ = gh(
            &["repo", "fork", UPSTREAM, "--clone=false", "--remote=false"],
            None,
        );
        let fork = format!("{login}/{name}");
        let parent = gh(
            &["api", &format!("repos/{fork}"), "--jq", ".parent.full_name"],
            None,
        )
        .map_err(|_| format!("cannot find your fork at {fork}"))?;
        if !parent.eq_ignore_ascii_case(UPSTREAM) {
            return Err(format!(
                "{fork} is not a fork of {UPSTREAM}; rename that repository or fork {UPSTREAM} by hand"
            ));
        }
        fork
    };
    let branch = format!("plugin/{}-{}", entry.id, entry.version);
    let refs = format!("repos/{head_repo}/git/refs");
    let create = || {
        gh(
            &[
                "api",
                "-X",
                "POST",
                &refs,
                "-f",
                &format!("ref=refs/heads/{branch}"),
                "-f",
                &format!("sha={base}"),
            ],
            None,
        )
    };
    let mut created = create();
    // A fork made a moment ago can take a few seconds to hold its commits.
    for _ in 0..5 {
        let retry = match &created {
            Ok(_) => false,
            Err(error) => !error.contains("already exists"),
        };
        if !retry {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
        created = create();
    }
    if created
        .as_ref()
        .is_err_and(|error| error.contains("already exists"))
    {
        created = gh(
            &[
                "api",
                "-X",
                "PATCH",
                &format!("{refs}/heads/{branch}"),
                "-f",
                &format!("sha={base}"),
                "-F",
                "force=true",
            ],
            None,
        );
    }
    created.map_err(|error| format!("creating the branch {branch} on {head_repo}: {error}"))?;
    let message = title(entry, &change);
    let put = serde_json::json!({
        "message": message,
        "content": base64::engine::general_purpose::STANDARD.encode(next.as_bytes()),
        "sha": blob,
        "branch": branch,
    });
    gh(
        &[
            "api",
            "-X",
            "PUT",
            &format!("repos/{head_repo}/contents/{REGISTRY_FILE}"),
            "--input",
            "-",
        ],
        Some(put.to_string().as_bytes()),
    )?;
    let head = if head_repo == UPSTREAM {
        branch.clone()
    } else {
        format!("{login}:{branch}")
    };
    let body = body(prepared, &change);
    match gh(
        &[
            "pr", "create", "--repo", UPSTREAM, "--base", "main", "--head", &head, "--title",
            &message, "--body", &body,
        ],
        None,
    ) {
        Ok(url) => Ok(url.lines().last().unwrap_or(&url).to_owned()),
        Err(error) if error.contains("already exists") => {
            let url = gh(
                &[
                    "pr", "list", "--repo", UPSTREAM, "--head", &branch, "--json", "url", "--jq",
                    ".[0].url",
                ],
                None,
            )?;
            Ok(format!("{url} (updated)"))
        }
        Err(error) => Err(error),
    }
}
