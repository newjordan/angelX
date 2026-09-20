use crate::agent::sandbox::process_owner::OwnedCommandExt;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

const SUMMARY_CACHE_LIMIT: usize = 128;

static SUMMARY_CACHE: OnceLock<Mutex<HashMap<PathBuf, &'static str>>> = OnceLock::new();

/// A rich-media card surfaced in the right-panel carousel: previews, links,
/// graphs, and resources the angel produces during a session.
#[allow(dead_code)]
pub enum Media {
    Image {
        label: String,
        path: String,
    },
    Video {
        label: String,
        path: String,
    },
    Link {
        label: String,
        url: String,
    },
    Graph {
        label: String,
        url: String,
    },
    Resource {
        label: String,
        url: String,
    },
    /// Model deliveries retain workspace authority through actual decoding.
    Confined {
        card: Box<Media>,
        root: PathBuf,
    },
}

impl Media {
    pub fn label(&self) -> &str {
        match self {
            Media::Image { label, .. }
            | Media::Video { label, .. }
            | Media::Link { label, .. }
            | Media::Graph { label, .. }
            | Media::Resource { label, .. } => label,
            Media::Confined { card, .. } => card.label(),
        }
    }

    pub fn sigil(&self) -> &'static str {
        match self {
            Media::Image { .. } => "img",
            Media::Video { .. } => "reel",
            Media::Link { .. } => "link",
            Media::Graph { .. } => "graph",
            Media::Resource { .. } => "res",
            Media::Confined { card, .. } => card.sigil(),
        }
    }

    pub fn target(&self) -> String {
        match self {
            Media::Link { url, .. } | Media::Resource { url, .. } => url.clone(),
            Media::Image { path, .. } | Media::Video { path, .. } => path.clone(),
            Media::Confined { card, .. } => card.target(),
            Media::Graph { url, .. } => {
                if url.is_empty() {
                    graph_fallback_url().to_string()
                } else {
                    url.clone()
                }
            }
        }
    }

    pub fn local_path(&self) -> Option<PathBuf> {
        let raw = match self {
            Media::Confined { card, .. } => return card.local_path(),
            Media::Image { path, .. } | Media::Video { path, .. } => path.as_str(),
            Media::Resource { url, .. } | Media::Graph { url, .. } | Media::Link { url, .. } => {
                url.as_str()
            }
        };
        local_media_path(raw)
    }

    pub fn is_image(&self) -> bool {
        match self {
            Media::Confined { card, .. } => card.is_image(),
            _ => matches!(self, Media::Image { .. }),
        }
    }

    pub fn is_video(&self) -> bool {
        match self {
            Media::Confined { card, .. } => card.is_video(),
            _ => matches!(self, Media::Video { .. }),
        }
    }

    pub fn is_visual(&self) -> bool {
        self.is_image() || self.is_video()
    }

    pub(crate) fn source(&self) -> Option<MediaSource> {
        match self {
            Media::Confined { card, root } => Some(MediaSource {
                path: card.local_path()?,
                root: Some(root.clone()),
            }),
            _ => Some(MediaSource::operator(self.local_path()?)),
        }
    }

    #[cfg(test)]
    fn artifact_summary(&self) -> String {
        self.artifact_summary_cow().into_owned()
    }

    pub fn artifact_summary_cow(&self) -> Cow<'_, str> {
        match self {
            // Summaries must not reopen a model path outside its authority.
            Media::Confined { .. } => Cow::Borrowed("workspace artifact"),
            Media::Image { path, .. } => {
                local_artifact_summary_raw(path).unwrap_or_else(|| truncated_target(path))
            }
            Media::Video { path, .. } => {
                local_artifact_summary_raw(path).unwrap_or_else(|| truncated_target(path))
            }
            Media::Resource { url, .. } => {
                local_artifact_summary_raw(url).unwrap_or_else(|| truncated_target(url))
            }
            Media::Link { url, .. } => truncated_target(url),
            Media::Graph { url, .. } => {
                if url.is_empty() {
                    Cow::Borrowed(graph_fallback_url())
                } else {
                    truncated_target(url)
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MediaSource {
    pub(crate) path: PathBuf,
    pub(crate) root: Option<PathBuf>,
}

impl MediaSource {
    pub(crate) fn operator(path: PathBuf) -> Self {
        Self { path, root: None }
    }

    /// Keep the returned descriptor alive through every consumer operation.
    pub(crate) fn open(&self) -> Result<std::fs::File, String> {
        reject_quarantine(&self.path)?;
        match &self.root {
            Some(root) => {
                let relative = crate::agent::harness::workspace_relative(root, &self.path)?;
                reject_quarantine(&relative)?;
                crate::agent::harness::confined_open_read_no_symlinks(root, &relative).map_err(|error| {
                    format!(
                        "Cannot read requested file (workspace symlinks are not allowed): {error}"
                    )
                })
            }
            None => {
                let path = self
                    .path
                    .canonicalize()
                    .map_err(|error| format!("Cannot read requested file: {error}"))?;
                reject_quarantine(&path)?;
                // The operator can name an external local file, but once its
                // identity is resolved a parent swap cannot redirect the open.
                #[cfg(unix)]
                {
                    crate::agent::harness::confined_open_read_no_symlinks(Path::new("/"), &path)
                }
                #[cfg(not(unix))]
                {
                    Err("descriptor-bound artifact display is unsupported on this platform".into())
                }
            }
        }
    }
}

fn reject_quarantine(path: &Path) -> Result<(), String> {
    if path
        .components()
        .any(|part| part.as_os_str() == "off-limits")
    {
        return Err("quarantined artifacts cannot be opened in the active Stage".into());
    }
    Ok(())
}

pub(crate) fn presentation_text_valid(text: &str, limit: usize) -> bool {
    !text.trim().is_empty() && text.len() <= limit
        && !text.chars().any(|ch| ch.is_control() || matches!(ch, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}'))
}

/// A successful `present` dispatch contains the exact workspace-bound target.
/// Error text and arbitrary JSON must never become a visible delivery event.
pub(crate) fn presentation_from_result(result: &str) -> Option<(String, String, String)> {
    let value: serde_json::Value = serde_json::from_str(result).ok()?;
    if value.get("status")?.as_str()? != "queued" {
        return None;
    }
    let kind = value.get("kind")?.as_str()?;
    let label = value.get("label")?.as_str()?;
    let url = value.get("url")?.as_str()?;
    if !["image", "video", "resource", "link", "graph"].contains(&kind)
        || !presentation_text_valid(label, 512)
        || !presentation_text_valid(url, 8192)
    {
        return None;
    }
    Some((kind.to_string(), label.to_string(), url.to_string()))
}

pub(crate) fn presentation_target_in(raw: &str, workspace: &Path) -> Result<String, String> {
    if !presentation_text_valid(raw, 8192) {
        return Err("target must contain 1–8192 bytes without control characters".into());
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Ok(raw.to_string());
    }
    if raw.contains("://") && !raw.starts_with("file://") {
        return Err("unsupported target scheme; use a local file or http(s) URL".into());
    }
    let path = local_media_path_in(raw, workspace).ok_or("target is not a local file")?;
    reject_quarantine(&path)?;
    Ok(path.to_string_lossy().into_owned())
}

pub(crate) fn model_presentation_target_in(raw: &str, workspace: &Path) -> Result<String, String> {
    let target = presentation_target_in(raw, workspace)?;
    if target.starts_with("http://") || target.starts_with("https://") {
        return Ok(target);
    }
    let relative = crate::agent::harness::workspace_relative(workspace, Path::new(&target))?;
    reject_quarantine(&relative)?;
    let root = workspace
        .canonicalize()
        .map_err(|error| format!("Cannot identify presentation workspace: {error}"))?;
    reject_quarantine(&root)?;
    Ok(root.join(relative).to_string_lossy().into_owned())
}

/// Operator `/show` accepts documents as well as the existing raster/MP4 path.
/// Reading and decoding happens off the UI thread; unknown formats retain their
/// actual identity and get an explicit unsupported-document result in Stage.
pub(crate) fn artifact_from_path_in(raw: &str, workspace: &Path) -> Result<Media, String> {
    let target = presentation_target_in(raw, workspace)?;
    if target.starts_with("http://") || target.starts_with("https://") {
        return Ok(Media::Link {
            label: "Remote source".into(),
            url: target,
        });
    }
    let path = Path::new(&target);
    if let Ok(canonical) = path.canonicalize() {
        reject_quarantine(&canonical)?;
    }
    if !path.is_file() {
        return Err(format!("missing artifact: {}", path.display()));
    }
    let label = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("artifact")
        .to_string();
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    Ok(match extension.as_str() {
        "mp4" | "mkv" | "mov" | "webm" | "avi" => Media::Video {
            label,
            path: target,
        },
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" | "tif" | "ico" | "pnm"
        | "qoi" => Media::Image {
            label,
            path: target,
        },
        _ => Media::Resource { label, url: target },
    })
}

fn local_artifact_summary_raw(raw: &str) -> Option<Cow<'static, str>> {
    let raw = raw.strip_prefix("file://").unwrap_or(raw);
    if raw.starts_with("http://") || raw.starts_with("https://") || raw.trim().is_empty() {
        return None;
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Some(local_artifact_summary_path(path));
    }
    local_media_path(raw).map(local_artifact_summary)
}

fn truncated_target(raw: &str) -> Cow<'_, str> {
    const LIMIT: usize = 24;
    if raw.len() <= LIMIT {
        return Cow::Borrowed(raw);
    }
    if raw.is_ascii() {
        return Cow::Borrowed(&raw[..LIMIT]);
    }
    for (count, (index, _)) in raw.char_indices().enumerate() {
        if count == LIMIT {
            return Cow::Borrowed(&raw[..index]);
        }
    }
    Cow::Borrowed(raw)
}

fn local_artifact_summary(path: PathBuf) -> Cow<'static, str> {
    local_artifact_summary_path(&path)
}

fn local_artifact_summary_path(path: &Path) -> Cow<'static, str> {
    let cache = SUMMARY_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock()
        && let Some(summary) = cache.get(path)
    {
        return Cow::Borrowed(summary);
    }

    if let Some((w, h)) = png_dimensions_fast(path) {
        let summary = format!("{w}x{h}");
        return cache_summary(cache, path, summary);
    }

    let Ok(meta) = std::fs::metadata(path) else {
        return Cow::Borrowed("missing");
    };
    let len = meta.len();

    let summary = image::image_dimensions(path)
        .map(|(w, h)| format!("{w}x{h}"))
        .unwrap_or_else(|_| format_bytes(len));
    cache_summary(cache, path, summary)
}

fn cache_summary(
    cache: &Mutex<HashMap<PathBuf, &'static str>>,
    path: &Path,
    summary: String,
) -> Cow<'static, str> {
    if let Ok(mut cache) = cache.lock()
        && cache.len() < SUMMARY_CACHE_LIMIT
    {
        let summary: &'static str = Box::leak(summary.into_boxed_str());
        cache.insert(path.to_path_buf(), summary);
        return Cow::Borrowed(summary);
    }
    Cow::Owned(summary)
}

fn png_dimensions_fast(path: &Path) -> Option<(u32, u32)> {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        return None;
    }

    let mut header = [0u8; 24];
    let mut file = std::fs::File::open(path).ok()?;
    file.read_exact(&mut header).ok()?;
    if &header[..8] != b"\x89PNG\r\n\x1a\n" || &header[12..16] != b"IHDR" {
        return None;
    }
    Some((
        u32::from_be_bytes(header[16..20].try_into().ok()?),
        u32::from_be_bytes(header[20..24].try_into().ok()?),
    ))
}

pub fn format_bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn open_target(target: &str) -> std::io::Result<()> {
    if std::env::var("WSL_DISTRO_NAME").is_ok() {
        let arg = if target.starts_with("http://") || target.starts_with("https://") {
            target.to_string()
        } else {
            Command::new("wslpath")
                .args(["-w", target])
                .output_owned()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| target.to_string())
        };
        Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", arg.as_str()])
            .spawn_owned()
            .map(|_| ())
            .or_else(|_| {
                Command::new("explorer.exe")
                    .arg(arg)
                    .spawn_owned()
                    .map(|_| ())
            })
    } else {
        Command::new("xdg-open")
            .arg(target)
            .spawn_owned()
            .map(|_| ())
    }
}

#[cfg(not(test))]
fn web_port() -> u16 {
    static WEB_PORT: OnceLock<u16> = OnceLock::new();
    *WEB_PORT.get_or_init(env_web_port)
}

#[cfg(test)]
fn web_port() -> u16 {
    env_web_port()
}

fn env_web_port() -> u16 {
    std::env::var("ANGEL_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5173)
}

fn graph_fallback_url() -> &'static str {
    static GRAPH_FALLBACK_URL: OnceLock<String> = OnceLock::new();
    GRAPH_FALLBACK_URL
        .get_or_init(|| format!("http://localhost:{}/", web_port()))
        .as_str()
}

/// Resolve only already-absolute local cards. Legacy relative cards are inert:
/// guessing via process cwd can cross the active project boundary.
fn local_media_path(raw: &str) -> Option<PathBuf> {
    let raw = raw.strip_prefix("file://").unwrap_or(raw);
    if raw.starts_with("http://") || raw.starts_with("https://") || raw.trim().is_empty() {
        return None;
    }
    let path = Path::new(raw);
    path.is_absolute().then(|| path.to_path_buf())
}

fn local_media_path_in(raw: &str, workspace: &Path) -> Option<PathBuf> {
    if let Some(path) = local_media_path(raw) {
        return Some(path);
    }
    let raw = raw.strip_prefix("file://").unwrap_or(raw);
    if raw.starts_with("http://") || raw.starts_with("https://") || raw.trim().is_empty() {
        return None;
    }
    Some(workspace.join(raw))
}

/// Classify a `/show` target for the terminal-native Scryglass. Video is kept
/// deliberately local and MP4-first; every other supported raster format is
/// verified through the image crate before a card is created.
pub fn visual_from_path(raw: &str) -> Result<Media, String> {
    visual_from_path_in(raw, &crate::agent::harness::default_workspace())
}

/// Resolve a visual against the cockpit's active workspace. Relative paths do
/// not fall back to the process cwd, which may name a different project after
/// `/cd`.
pub fn visual_from_path_in(raw: &str, workspace: &Path) -> Result<Media, String> {
    let path = local_media_path_in(raw, workspace)
        .ok_or_else(|| "target is not a local file".to_string())?;
    if !path.is_file() {
        return Err(format!("missing visual: {}", path.display()));
    }
    let label = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("visual")
        .to_string();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("mp4") {
        return Ok(Media::Video {
            label,
            path: path.to_string_lossy().into_owned(),
        });
    }
    image::image_dimensions(&path)
        .map_err(|error| format!("unsupported visual {}: {error}", path.display()))?;
    Ok(Media::Image {
        label,
        path: path.to_string_lossy().into_owned(),
    })
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/media__tests.rs"]
mod tests;
