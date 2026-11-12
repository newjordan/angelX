//! `/plugins`: the plugin manager inside the cockpit (docs/PLUGINS.md).
//!
//! What touches the network (search, info, install, update) runs as the
//! cockpit's background job, so Esc cancels it and the composer stays live;
//! a setup's output streams into the job's panel. What is local (the list,
//! remove, check) answers at once. After an install or a removal the
//! cartridges are read again, so a plugin's commands and skills work at once.

use super::*;
use crate::app::plugins::cli::{self, Failure, Surface};
use crate::app::plugins::{Paths, install::Output};

impl App {
    pub(crate) fn plugins_command(&mut self, arg: Option<&str>) {
        let mut words: Vec<String> = arg
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        anchor_folders(&mut words, self.tools.current_workspace());
        let paths = Paths::from_env();
        let network = matches!(
            words.first().map(String::as_str),
            Some(
                "search"
                    | "find"
                    | "info"
                    | "show"
                    | "install"
                    | "add"
                    | "update"
                    | "upgrade"
                    | "setup"
                    | "submit"
                    | "publish"
            )
        );
        if !network {
            let removes = matches!(
                words.first().map(String::as_str),
                Some("remove" | "uninstall" | "rm")
            );
            let links = words.first().is_some_and(|word| word == "link");
            let report = match cli::run(
                &paths,
                &words,
                Surface::Cockpit,
                &|_| {},
                Output::Inherit,
                None,
            ) {
                Ok(text) if removes => {
                    crate::agent::harness::cartridges::reload();
                    format!(
                        "{text}\n  its commands are gone now; its tools leave when angelX next starts"
                    )
                }
                Ok(text) if links => {
                    crate::agent::harness::cartridges::reload();
                    format!(
                        "{text}\n  its commands and skills work now; its tools join when angelX next starts"
                    )
                }
                Ok(text) => text,
                Err(Failure::Usage(text) | Failure::Failed(text)) => text,
            };
            self.system_msg(report);
            return;
        }
        if self.thinking.is_some() || self.bg_job.is_some() {
            self.system_msg(
                "/plugins: the flight slot is busy — retry after the current turn or job lands"
                    .to_string(),
            );
            return;
        }
        let installs = matches!(
            words[0].as_str(),
            "install" | "add" | "update" | "upgrade" | "setup"
        );
        let (tx, job) = BackgroundJob::channel("plugins", "Retry /plugins");
        let started = format!("/plugins {} … (Esc cancels)", words.join(" "));
        std::thread::spawn(move || {
            if tx.is_cancelled() {
                return;
            }
            let progress = tx.output_progress();
            let say = {
                let progress = Arc::clone(&progress);
                move |line: &str| {
                    progress(
                        crate::agent::harness::ProcessStream::Stdout,
                        format!("{line}\n").as_bytes(),
                    )
                }
            };
            let sink = {
                let progress = Arc::clone(&progress);
                move |bytes: &[u8]| progress(crate::agent::harness::ProcessStream::Stdout, bytes)
            };
            let result = cli::run(
                &paths,
                &words,
                Surface::Cockpit,
                &say,
                Output::Sink(&sink),
                Some(tx.cancellation_flag()),
            );
            let mut text = match result {
                Ok(text) => text,
                Err(Failure::Usage(text) | Failure::Failed(text)) => text,
            };
            if installs {
                crate::agent::harness::cartridges::reload();
                let commands: Vec<String> = crate::agent::harness::cartridges::commands()
                    .filter(|command| crate::ui::input::cartridge_command(&command.name).is_some())
                    .map(|command| format!("/{}", command.name))
                    .collect();
                if text.contains("installed") || text.contains("updated") {
                    text.push_str("\n  its commands and skills work now; its tools join when angelX next starts");
                    if !commands.is_empty() {
                        text.push_str(&format!(" · plugged-in commands: {}", commands.join(" ")));
                    }
                }
            }
            let _ = tx.send(BgOutcome::Note(text));
        });
        self.bg_job = Some(job);
        self.system_msg(started);
    }
}

/// Folder arguments name folders in the session's workspace, not wherever the
/// cockpit process happens to stand: `check`, `link` and `submit` take the
/// workspace itself when given none, `new <id>` makes `<workspace>/<id>`.
pub(crate) fn anchor_folders(words: &mut Vec<String>, workspace: &Path) {
    let Some(command) = words.first().cloned() else {
        return;
    };
    let anchor = |word: &str| -> String {
        let path = Path::new(word);
        if path.is_absolute() {
            word.to_owned()
        } else {
            workspace.join(path).display().to_string()
        }
    };
    // The positional words after the command, skipping flags and their values.
    let valued = ["--commit", "--repository", "--into"];
    let mut positional = Vec::new();
    let mut at = 1;
    while at < words.len() {
        let word = &words[at];
        if valued.contains(&word.as_str()) {
            if word == "--into" && at + 1 < words.len() {
                words[at + 1] = anchor(&words[at + 1]);
            }
            at += 2;
            continue;
        }
        if !word.starts_with("--") {
            positional.push(at);
        }
        at += 1;
    }
    match command.as_str() {
        "check" | "lint" | "link" | "submit" | "publish" => match positional.first() {
            Some(&index) => words[index] = anchor(&words[index]),
            None => words.insert(1, workspace.display().to_string()),
        },
        "new" | "init" => match positional.as_slice() {
            [_, dir, ..] => words[*dir] = anchor(&words[*dir]),
            [id] => {
                let id = words[*id].clone();
                words.push(workspace.join(&id).display().to_string());
            }
            [] => {}
        },
        _ => {}
    }
}
