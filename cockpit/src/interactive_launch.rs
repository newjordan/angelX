//! Credential-free interactive entry preflight. Machine modes own their remaining
//! argv; values in those grammars must never be scanned as mode selectors.
use std::ffi::{OsStr, OsString};
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) struct InteractiveLaunch {
    pub workspace: Option<PathBuf>,
    pub draft: Option<String>,
    pub prompt: Option<String>,
    pub driver: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub resume: Option<Option<String>>,
}

fn invalid(message: &'static str) -> Error {
    Error::new(ErrorKind::InvalidInput, message)
}

pub(crate) fn ordinary_args(args: &[OsString]) -> &[OsString] {
    let leading = args
        .iter()
        .take_while(|arg| {
            matches!(
                arg.to_str(),
                Some("--yolo" | "--comp" | "--lean" | "--turbo" | "--angelturbo")
            )
        })
        .count();
    &args[leading..]
}

pub(crate) fn machine_mode(arg: &OsStr) -> bool {
    matches!(
        arg.to_str(),
        Some(
            "--atlas"
                | "--look-image"
                | "--tool-http-helper"
                | "--sandbox-exec"
                | "--audit-coding-eval-training"
                | "--build-info"
                | "--bind-graph-reward"
                | "--jev-json"
                | "--comp-status"
                | "--watch-fixture"
                | "--export-harness-rollout"
                | "--audit-harness-rollout"
                | "--dump-preview"
                | "--dump-preview-portrait"
                | "--dump-research-preview"
                | "--dump-rl-preview"
                | "--dump-scryglass"
                | "--dump-visual"
                | "--loop"
                | "--rollout"
                | "--ask"
                | "--board-sync"
                | "--task"
                | "--task-json"
        )
    )
}

/// None denotes an existing helper/headless grammar, not permission to enter a
/// TUI with unknown arguments. Errors deliberately never include argv content.
pub(crate) fn parse(args: &[OsString]) -> Result<Option<InteractiveLaunch>> {
    let ordinary = ordinary_args(args);
    let globals = &args[..args.len() - ordinary.len()];
    if globals.iter().filter(|arg| *arg == "--yolo").count() > 1
        || globals.iter().filter(|arg| *arg != "--yolo").count() > 1
    {
        return Err(invalid("duplicate global option"));
    }
    let args = ordinary;
    if args.first().is_some_and(|arg| machine_mode(arg)) {
        return Ok(None);
    }
    if args.first().is_some_and(|arg| {
        matches!(
            arg.to_str(),
            Some("--doctor" | "--help" | "-h" | "--version" | "-V")
        )
    }) {
        return if args.len() == 1 {
            Ok(None)
        } else {
            Err(invalid("inspection options take no additional arguments"))
        };
    }
    let mut launch = InteractiveLaunch::default();
    let mut args = args.iter().peekable();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--workspace") => {
                if launch.workspace.is_some() {
                    return Err(invalid("duplicate --workspace"));
                }
                let value = args
                    .next()
                    .ok_or_else(|| invalid("--workspace requires a directory"))?;
                if value.is_empty() {
                    return Err(invalid("--workspace requires a directory"));
                }
                launch.workspace = Some(PathBuf::from(value));
            }
            Some("--draft" | "--prompt") => {
                if launch.draft.is_some() || launch.prompt.is_some() {
                    return Err(invalid("duplicate initial text"));
                }
                let text = args
                    .next()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| invalid("initial input requires UTF-8 text"))?;
                if text.trim().is_empty() {
                    return Err(invalid("initial text must not be blank"));
                }
                if text.len() > crate::app::control::MAX_COMPOSER_PASTE_BYTES {
                    return Err(invalid("initial text exceeds the 256 KiB composer limit"));
                }
                if arg == "--draft" {
                    launch.draft = Some(text.to_owned());
                } else {
                    launch.prompt = Some(text.to_owned());
                }
            }
            Some("--resume") => {
                if launch.resume.is_some() {
                    return Err(invalid("duplicate --resume"));
                }
                let id = if args
                    .peek()
                    .is_some_and(|value| value.to_str().is_some_and(|text| !text.starts_with('-')))
                {
                    Some(
                        args.next()
                            .unwrap()
                            .to_str()
                            .filter(|text| !text.trim().is_empty() && text.len() <= 1024)
                            .ok_or_else(|| {
                                invalid("resume id must be nonblank UTF-8 within 1024 bytes")
                            })?
                            .to_owned(),
                    )
                } else {
                    None
                };
                launch.resume = Some(id);
            }
            Some("--driver" | "--model" | "--effort") => {
                let target = match arg.to_str().unwrap() {
                    "--driver" => &mut launch.driver,
                    "--model" => &mut launch.model,
                    _ => &mut launch.effort,
                };
                if target.is_some() {
                    return Err(invalid("duplicate launch selector"));
                }
                let value = args.next().and_then(|value| value.to_str())
                    .filter(|s| !s.trim().is_empty() && s.len() <= 1024 && s.trim() == *s)
                    .ok_or_else(|| invalid("launch selector requires nonblank UTF-8 within 1024 bytes, without surrounding whitespace"))?;
                *target = Some(value.to_owned());
            }
            _ => {
                return Err(invalid("unknown or trailing interactive argument"));
            }
        }
    }
    if launch.model.is_some() && launch.driver.is_none() {
        return Err(invalid("--model requires an explicit concrete --driver"));
    }
    if (launch.draft.is_some() || launch.prompt.is_some()) && launch.resume.is_some() {
        return Err(invalid("initial text cannot be combined with --resume"));
    }
    Ok(Some(launch))
}

/// Resolve relative selections against the original cwd, not a later bootstrap
/// directory. CLI > invoking environment > original cwd. No writes or auth.
pub(crate) fn workspace(
    launch: &InteractiveLaunch,
    env: Option<OsString>,
    cwd: &Path,
) -> Result<PathBuf> {
    let selected = launch
        .workspace
        .clone()
        .or_else(|| env.filter(|v| !v.is_empty()).map(PathBuf::from))
        .unwrap_or_else(|| cwd.to_path_buf());
    let selected = if selected.is_absolute() {
        selected
    } else {
        cwd.join(selected)
    };
    let root = selected
        .canonicalize()
        .map_err(|_| invalid("workspace must be an existing accessible directory"))?;
    if !root.is_dir() {
        return Err(invalid("workspace must be a directory"));
    }
    Ok(root)
}

#[cfg(test)]
#[path = "../../tests/cockpit/interactive_launch__tests.rs"]
mod tests;
