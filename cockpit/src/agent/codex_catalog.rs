//! Read-only startup metadata. No authentication, endpoint configuration or refresh.
use super::openai_codex::{CodexModelInfo, CodexReasoningLevel, is_private_test_codex_model};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

const MAX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_MODELS: usize = 2000;
const LEVELS: &[&str] = &["minimal", "low", "medium", "high", "xhigh", "max"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Legacy,
    PiStore,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Diagnostic {
    Selector,
    Open,
    NotRegular,
    TooLarge,
    Changed,
    Read,
    Shape,
    Identity,
    Duplicate,
    Capacity,
    Thinking,
    NoSelectableModels,
}
impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpenAI catalog failure: {self:?} (no fallback)")
    }
}

#[derive(Debug)]
pub(crate) struct CatalogLoad {
    pub source: Source,
    pub checked: Result<CheckedCatalog, Diagnostic>,
}
#[derive(Debug)]
pub(crate) struct CheckedCatalog {
    pub models: Vec<CodexModelInfo>,
    pub hash: String,
}
#[derive(Clone, Debug)]
pub(crate) struct EffectiveSelection {
    pub requested: Option<String>,
    pub wire: Option<String>,
    pub effort_source: Option<&'static str>,
}

pub(crate) fn checked_context(window: u64) -> Result<usize, Diagnostic> {
    let window = usize::try_from(window).map_err(|_| Diagnostic::Capacity)?;
    if window < 2560 {
        return Err(Diagnostic::Capacity);
    }
    Ok(window)
}

impl CatalogLoad {
    #[cfg(test)]
    pub fn load(selector: Option<&str>, legacy: impl FnOnce() -> Vec<CodexModelInfo>) -> Self {
        Self::load_checked(selector, || Ok(legacy()))
    }
    pub fn load_checked(
        selector: Option<&str>,
        legacy: impl FnOnce() -> Result<Vec<CodexModelInfo>, Diagnostic>,
    ) -> Self {
        match selector {
            None => Self {
                source: Source::Legacy,
                checked: legacy().map(normalize),
            },
            Some(value) => Self {
                source: Source::PiStore,
                checked: value
                    .strip_prefix("pi-store:")
                    .filter(|path| Path::new(path).is_absolute())
                    .ok_or(Diagnostic::Selector)
                    .and_then(|path| read_checked(Path::new(path))),
            },
        }
    }
    #[cfg(test)]
    pub fn from_models(source: Source, models: Vec<CodexModelInfo>) -> Self {
        Self {
            source,
            checked: Ok(normalize(models)),
        }
    }
    pub fn models(&self) -> &[CodexModelInfo] {
        self.checked
            .as_ref()
            .map(|c| c.models.as_slice())
            .unwrap_or(&[])
    }
    pub fn status(&self) -> Value {
        json!({"source": match self.source { Source::Legacy => "codex-cache", Source::PiStore => "pi-store" },
            "adapter": "angel-codex-catalog/1", "hash": self.checked.as_ref().ok().map(|c| &c.hash),
            "error": self.checked.as_ref().err().map(ToString::to_string),
            "adoption": "restart", "publisher_origin": "unverified"})
    }
    pub fn resolve(
        &self,
        model: &str,
        requested: Option<&str>,
        fallback: bool,
    ) -> Result<EffectiveSelection, String> {
        let checked = self.checked.as_ref().map_err(ToString::to_string)?;
        let requested = requested.map(|s| s.trim().to_ascii_lowercase());
        let spec = checked.models.iter().find(|m| m.slug == model);
        if spec.is_none()
            && !(self.source == Source::Legacy && checked.models.is_empty() && fallback)
        {
            return Err(format!(
                "OpenAI model {model:?} is not supported by selected catalog"
            ));
        }
        if let Some(window) = spec.and_then(|s| s.context_window) {
            checked_context(window).map_err(|e| format!("{e}: model context capacity"))?;
        }
        let wire = match requested.as_deref() {
            Some(intent) if intent.starts_with("pi:") => {
                if self.source != Source::PiStore {
                    return Err("Pi effort requires a Pi catalog".into());
                }
                let level = &intent[3..];
                if !LEVELS.contains(&level) {
                    return Err("unsupported Pi thinking level".into());
                }
                let spec = spec.ok_or("Pi effort requires checked model")?;
                match spec.pi_thinking.get(level) {
                    Some(Some(wire)) => Some(wire.clone()),
                    Some(None) => return Err(format!("Pi thinking {level:?} is excluded")),
                    None if level == "xhigh" || level == "max" => {
                        return Err(format!("Pi thinking {level:?} requires mapping"));
                    }
                    None => Some(level.to_string()),
                }
            }
            Some("ultra") => Some("xhigh".into()),
            Some(native) => Some(native.to_string()),
            None => None,
        };
        if let Some(wire) = &wire {
            let supported = spec.map_or_else(
                || fallback && wire == super::openai_codex::OPENAI_LUNA_EFFORT,
                |s| {
                    s.supported_reasoning_levels
                        .iter()
                        .any(|l| l.effort == *wire || l.effort == "ultra" && wire == "xhigh")
                },
            );
            if !supported {
                let levels = spec
                    .map(|s| {
                        s.supported_reasoning_levels
                            .iter()
                            .map(|l| l.effort.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_else(|| super::openai_codex::OPENAI_LUNA_EFFORT.into());
                return Err(format!(
                    "OpenAI effort {wire:?} is not supported for {model}; supported efforts: [{levels}]"
                ));
            }
        }
        Ok(EffectiveSelection {
            requested,
            wire,
            effort_source: None,
        })
    }
}

fn revision(m: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
fn read_checked(path: &Path) -> Result<CheckedCatalog, Diagnostic> {
    read_checked_with(path, |_| {})
}
fn read_checked_with(
    path: &Path,
    mut after_read: impl FnMut(usize),
) -> Result<CheckedCatalog, Diagnostic> {
    // Reopen only after a revision change; consumers never read again.
    for attempt in 0..2 {
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| Diagnostic::Open)?;
        let before = file.metadata().map_err(|_| Diagnostic::Read)?;
        if !before.is_file() {
            return Err(Diagnostic::NotRegular);
        }
        if before.len() > MAX_BYTES {
            return Err(Diagnostic::TooLarge);
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Diagnostic::Read)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(Diagnostic::TooLarge);
        }
        after_read(attempt);
        let after = file.metadata().map_err(|_| Diagnostic::Read)?;
        let current = std::fs::metadata(path).ok();
        if revision(&before) != revision(&after)
            || current.as_ref().map(revision) != Some(revision(&after))
        {
            if attempt == 0 {
                continue;
            }
            return Err(Diagnostic::Changed);
        }
        return parse(&bytes);
    }
    Err(Diagnostic::Changed)
}
fn text<'a>(record: &'a Value, key: &str, max: usize) -> Result<&'a str, Diagnostic> {
    record
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control))
        .ok_or(Diagnostic::Shape)
}
fn positive(record: &Value, key: &str) -> Result<u64, Diagnostic> {
    record
        .get(key)
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
        .ok_or(Diagnostic::Capacity)
}
fn parse(bytes: &[u8]) -> Result<CheckedCatalog, Diagnostic> {
    let root: Value = serde_json::from_slice(bytes).map_err(|_| Diagnostic::Shape)?;
    let records = root
        .get("openai-codex")
        .and_then(|p| p.get("models"))
        .and_then(Value::as_array)
        .ok_or(Diagnostic::Shape)?;
    if records.len() > MAX_MODELS {
        return Err(Diagnostic::TooLarge);
    }
    let mut ids = BTreeSet::new();
    let mut models = Vec::new();
    for r in records {
        if text(r, "provider", 64)? != "openai-codex"
            || text(r, "api", 64)? != "openai-codex-responses"
        {
            return Err(Diagnostic::Identity);
        }
        let id = text(r, "id", 256)?;
        if !ids.insert(id) {
            return Err(Diagnostic::Duplicate);
        }
        let name = text(r, "name", 320)?;
        let window = positive(r, "contextWindow")?;
        checked_context(window)?;
        usize::try_from(positive(r, "maxTokens")?).map_err(|_| Diagnostic::Capacity)?;
        let inputs = r
            .get("input")
            .and_then(Value::as_array)
            .ok_or(Diagnostic::Shape)?;
        if inputs.is_empty()
            || inputs.len() > 2
            || inputs
                .iter()
                .any(|v| !matches!(v.as_str(), Some("text" | "image")))
        {
            return Err(Diagnostic::Shape);
        }
        let input_modalities = inputs
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        if input_modalities.iter().collect::<BTreeSet<_>>().len() != input_modalities.len() {
            return Err(Diagnostic::Duplicate);
        }
        let reasoning = r
            .get("reasoning")
            .and_then(Value::as_bool)
            .ok_or(Diagnostic::Thinking)?;
        let mut thinking = BTreeMap::new();
        if let Some(map) = r.get("thinkingLevelMap") {
            let map = map.as_object().ok_or(Diagnostic::Thinking)?;
            for (level, wire) in map {
                // Pi publishes both exclusions and a native no-effort wire
                // under `off`. Neither is selectable by this reasoning route.
                if level == "off" && (wire.is_null() || wire.as_str() == Some("none")) {
                    continue;
                }
                if !LEVELS.contains(&level.as_str()) {
                    return Err(Diagnostic::Thinking);
                }
                let wire = if wire.is_null() {
                    None
                } else {
                    Some(
                        wire.as_str()
                            .filter(|w| LEVELS.contains(w))
                            .ok_or(Diagnostic::Thinking)?
                            .to_string(),
                    )
                };
                thinking.insert(level.clone(), wire);
            }
        }
        if is_private_test_codex_model(id)
            || !reasoning
            || !input_modalities.iter().any(|v| v == "text")
        {
            continue;
        }
        // Fixed Codex wire profile, never imported generic compat flags or clamping.
        let mut wires = BTreeSet::new();
        for level in LEVELS {
            match thinking.get(*level) {
                Some(Some(wire)) => {
                    wires.insert(wire.clone());
                }
                Some(None) => {}
                None if *level != "xhigh" && *level != "max" => {
                    wires.insert(level.to_string());
                }
                None => {}
            }
        }
        if wires.is_empty() {
            continue;
        }
        models.push(CodexModelInfo {
            slug: id.into(),
            display_name: name.into(),
            description: name.into(),
            context_window: Some(window),
            input_modalities,
            supported_reasoning_levels: LEVELS
                .iter()
                .filter(|level| wires.contains(**level))
                .map(|level| CodexReasoningLevel {
                    effort: (*level).to_string(),
                    description: String::new(),
                })
                .collect(),
            pi_thinking: thinking,
            ..Default::default()
        });
    }
    if models.is_empty() {
        return Err(Diagnostic::NoSelectableModels);
    }
    Ok(normalize(models))
}
fn normalize(models: Vec<CodexModelInfo>) -> CheckedCatalog {
    let normalized = models.iter().map(|m| json!({"id": m.slug, "name": m.display_name,
        "context": m.context_window, "input": m.input_modalities, "default": m.default_reasoning_level,
        "levels": m.supported_reasoning_levels.iter().map(|l| &l.effort).collect::<Vec<_>>(), "thinking": m.pi_thinking})).collect::<Vec<_>>();
    let digest = ring::digest::digest(
        &ring::digest::SHA256,
        &serde_json::to_vec(&normalized).expect("normalized JSON"),
    );
    CheckedCatalog {
        models,
        hash: digest.as_ref().iter().map(|b| format!("{b:02x}")).collect(),
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/codex_catalog__tests.rs"]
mod tests;
