//! Slash-command help text. Command parsing and execution remain native.
use serde::Deserialize;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, OnceLock};

const EMBEDDED: &str = include_str!("../../../lib/control/slash-help.json");
const MAX_BYTES: u64 = 128 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Help {
    version: u32,
    first: String,
    before_atlas: String,
    atlas: String,
    after_atlas: String,
}

impl Help {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let help: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid help catalog: {error}"))?;
        if help.version != 1
            || !help.first.contains("{b}")
            || [
                &help.first,
                &help.before_atlas,
                &help.atlas,
                &help.after_atlas,
            ]
            .iter()
            .any(|text| text.is_empty() || text.contains('\0'))
        {
            return Err("invalid help catalog fields".into());
        }
        Ok(help)
    }

    pub(super) fn render(&self, topic: Option<&str>, bullet: &str, atlas_disabled: bool) -> String {
        let first = || self.first.replace("{b}", bullet);
        let full = || {
            let text = format!("{}{}{}", self.before_atlas, self.atlas, self.after_atlas);
            if atlas_disabled {
                text.replace(
                    "     /atlas [query] · reviewed project knowledge · add/accept/revise/reject/\n\
                     promote/challenge/forget/tend · observe <path> [symbol] · link <id> <related|derived_from> <target> · trace <id> · /vault opens its Artifacts lane\n",
                    "",
                )
            } else {
                text
            }
        };
        match topic.map(str::trim).filter(|topic| !topic.is_empty()) {
            None | Some("1") | Some("index") => first(),
            Some("all") => full(),
            Some(topic) => {
                let key = topic.to_ascii_lowercase();
                let text = full();
                if text.to_ascii_lowercase().contains(&format!("/{key}")) {
                    format!("commands · {key}\n{text}")
                } else {
                    format!("commands · unknown topic {topic}\n{}", first())
                }
            }
        }
    }
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err("help catalog is not a bounded regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("help catalog exceeds byte limit".into());
    }
    Ok(bytes)
}

fn capture(path: Option<&Path>) -> Result<Arc<Help>, String> {
    static BASELINE: OnceLock<Arc<Help>> = OnceLock::new();
    let Some(path) = path else {
        return Ok(Arc::clone(BASELINE.get_or_init(|| {
            Arc::new(Help::parse(EMBEDDED.as_bytes()).expect("valid embedded slash help"))
        })));
    };
    if !path.is_absolute() {
        return Err("help catalog path must be absolute".into());
    }
    let bytes = read_bounded(path)?;
    let help = Help::parse(&bytes)?;
    if read_bounded(path)? != bytes {
        return Err("help catalog changed during capture".into());
    }
    Ok(Arc::new(help))
}

pub(super) fn load() -> Result<Arc<Help>, String> {
    let path = std::env::var_os("ANGEL_SLASH_HELP_CATALOG").filter(|value| !value.is_empty());
    capture(path.as_deref().map(Path::new))
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/slash_help__tests.rs"]
mod tests;
