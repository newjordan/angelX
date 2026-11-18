//! Vision sidecar for text-only clubs (DeepSeek V4 Flash and friends).
//!
//! Community pattern (X, 2026-08): DeepSeek Flash has no image input, so
//! people bolt a small OpenAI-compatible VLM in front — describe frames to
//! text, then let the main model reason. Qwen-MM-Plugins is the popular full
//! plugin stack; the lightweight fit for angel is the same idea we already
//! use for `video_look`: `ANGEL_VISION_URL` + `ANGEL_VISION_MODEL` (+ key).
//! An in-hand model that accepts images looks with itself. A text-only driver
//! falls through to the signed-in Codex image model. Kimi credentials are not
//! a vision route.
//!
//! This module:
//! - resolves that vision backend once
//! - registers `vision_look` (still images + video frames) whenever a backend
//!   is configured, independent of the video-edit tool suite
//! - rewrites `/see` attachments into text when the in-hand club is text-only
//!   so DeepSeek can finish vision jobs without a 400

use crate::agent::club::{ChatMsg, ChatRole, Club, ClubReply, HttpClub, Media, ToolDef};
use crate::agent::harness::{Tool, env_flag};
use crate::agent::sandbox::SandboxPolicy;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const MEDIA_DURATION_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub(crate) struct VisionBackend(pub Result<Arc<dyn Club>, String>);
impl VisionBackend {
    pub(crate) fn new(run: Option<&crate::agent::codex_startup::CodexStartup>) -> Self {
        Self::for_driver(run, None)
    }

    pub(crate) fn for_driver(
        run: Option<&crate::agent::codex_startup::CodexStartup>,
        in_hand: Option<&Arc<dyn Club>>,
    ) -> Self {
        Self(resolve_vision_club_in(run, in_hand))
    }
    pub(crate) fn configured(&self) -> bool {
        self.0.is_ok()
    }
}

fn resolve_vision_club_in(
    run: Option<&crate::agent::codex_startup::CodexStartup>,
    in_hand: Option<&Arc<dyn Club>>,
) -> Result<Arc<dyn Club>, String> {
    if let (Some(url), Some(model)) = (
        std::env::var("ANGEL_VISION_URL").ok(),
        std::env::var("ANGEL_VISION_MODEL").ok(),
    ) {
        let key = std::env::var("ANGEL_VISION_KEY").ok();
        return Ok(Arc::new(HttpClub::new("vision", url, model, key)));
    }
    if let Some(club) = in_hand
        .filter(|club| club_can_read_images(club.as_ref()) && !is_kimi_seat(club.as_ref()))
    {
        return Ok(Arc::clone(club));
    }
    run.ok_or("no vision backend configured: no run Codex context")?
        .vision_club()
        .map_err(|e| format!("no vision backend configured ({e})"))
}

fn vision_sidecar_enabled_in(backend: &VisionBackend) -> bool {
    env_flag("ANGEL_VISION_SIDECAR", true) && backend.configured()
}

pub(crate) fn club_is_text_only_for_vision(club: &dyn Club) -> bool {
    let meta = club.route_metadata();
    if !meta.input_modalities.is_empty() {
        return !meta
            .input_modalities
            .iter()
            .any(|m| m.eq_ignore_ascii_case("image"));
    }
    let label = club.label().to_ascii_lowercase();
    let model = club
        .model_identity()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let hay = format!("{label} {model}");
    if vision_capable_name(&hay) {
        return false;
    }
    text_only_name(&hay)
}

fn club_can_read_images(club: &dyn Club) -> bool {
    let meta = club.route_metadata();
    if !meta.input_modalities.is_empty() {
        return meta
            .input_modalities
            .iter()
            .any(|m| m.eq_ignore_ascii_case("image"));
    }
    let label = club.label().to_ascii_lowercase();
    let model = club
        .model_identity()
        .unwrap_or_default()
        .to_ascii_lowercase();
    vision_capable_name(&format!("{label} {model}"))
}

fn is_kimi_seat(club: &dyn Club) -> bool {
    let label = club.label().to_ascii_lowercase();
    let model = club
        .model_identity()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let hay = format!("{label} {model}");
    hay.contains("kimi") || hay.contains("moonshot")
}

fn vision_capable_name(hay: &str) -> bool {
    [
        "vision",
        "-vl",
        "vl-",
        "vlava",
        "llava",
        "gemma-3",
        "gemma3",
        "gpt-4o",
        "gpt-4.1",
        "gpt-5",
        "gpt-6",
        "claude-3",
        "claude-4",
        "gemini",
        "pixtral",
        "qwen2.5-vl",
        "qwen2-vl",
        "qwen-vl",
    ]
    .iter()
    .any(|needle| hay.contains(needle))
}

fn text_only_name(hay: &str) -> bool {
    [
        "deepseek-v4-flash",
        "deepseek-v4-pro",
        "deepseek-chat",
        "deepseek-reasoner",
        "longcat",
        "ling-",
        "ring-",
        "gpt-oss",
        "nemotron",
    ]
    .iter()
    .any(|needle| hay.contains(needle))
}

pub(crate) fn vision_sidecar_forced() -> bool {
    matches!(
        std::env::var("ANGEL_VISION_SIDECAR")
            .ok()
            .as_deref()
            .map(str::trim)
            .map(|v| v.to_ascii_lowercase()),
        Some(v) if v == "force" || v == "always"
    )
}

pub(crate) fn should_apply_vision_sidecar_in(
    backend: &VisionBackend,
    club: &dyn Club,
    msg: &ChatMsg,
) -> bool {
    if !vision_sidecar_enabled_in(backend) && !vision_sidecar_forced() {
        return false;
    }
    if !backend.configured() {
        return false;
    }
    let has_image = msg
        .attachments
        .iter()
        .any(|m| matches!(m, Media::Image { .. }));
    if !has_image {
        return false;
    }
    if vision_sidecar_forced() {
        return true;
    }
    club_is_text_only_for_vision(club)
}

pub(crate) fn describe_media_in(
    backend: &VisionBackend,
    media: Vec<Media>,
    question: &str,
) -> Result<(String, String), String> {
    #[cfg(test)]
    DESCRIBE_MEDIA_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if media.is_empty() {
        return Err("no media to describe".to_string());
    }
    let question = question.trim();
    if question.is_empty() {
        return Err("question cannot be empty".to_string());
    }
    let club = vision_describe_club(backend)?;
    let msg = ChatMsg::user_with_media(question, media);
    let workspace = crate::agent::harness::book::connect::workspace();
    let reply =
        crate::agent::harness::book::connect::converse(&workspace, &[msg], |history, tools| {
            club.chat(history, tools)
        })
        .map_err(|e| format!("vision sidecar ({}): {e}", club.label()))?;
    let text = match reply {
        ClubReply::Text(t) => t,
        ClubReply::Calls(_) => {
            "vision club answered with tool calls (unsupported for sidecar)".to_string()
        }
    };
    Ok((club.label().to_string(), text.trim().to_string()))
}

fn vision_describe_club(backend: &VisionBackend) -> Result<Arc<dyn Club>, String> {
    #[cfg(test)]
    {
        if let Some(club) = test_vision_club_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            return Ok(club);
        }
    }
    backend.0.clone()
}

pub(crate) fn image_cli(mut args: impl Iterator<Item = std::ffi::OsString>) -> std::io::Result<()> {
    let invalid = |message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message);
    let usage = "usage: angel --look-image IMAGE QUESTION";
    let path = args.next().ok_or_else(|| invalid(usage))?;
    let question = args
        .next()
        .ok_or_else(|| invalid(usage))?
        .into_string()
        .map_err(|_| invalid("question must be UTF-8"))?;
    if args.next().is_some() || question.trim().is_empty() {
        return Err(invalid(usage));
    }
    let media = Media::image_from_path(Path::new(&path))
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let (backend, description) = describe_media_in(
        &VisionBackend::new(Some(&crate::agent::codex_startup::CodexStartup::load())),
        vec![media],
        &question,
    )
    .map_err(std::io::Error::other)?;
    println!("[vision_look via {backend} · 1 image]\n{description}");
    Ok(())
}

pub(crate) fn sidecar_question(operator: &str) -> String {
    use crate::agent::harness::book::{d3_roles::pages, d456_knowledge::VISION};
    if operator.is_empty() {
        pages(VISION, [1, 2])
    } else {
        format!(
            "{}\n{operator}\n\n{}",
            pages(VISION, [3]),
            pages(VISION, [4])
        )
    }
}

pub(crate) fn sidecar_body(backend: &str, n: usize, description: &str, operator: &str) -> String {
    use crate::agent::harness::book::{d3_roles::pages, d456_knowledge::VISION};
    let mut body = format!("{} {backend} · {n}\n{description}", pages(VISION, [5]));
    if !operator.is_empty() {
        body.push_str(&format!("\n\n{} ", pages(VISION, [7])));
        body.push_str(operator);
    }
    body
}

fn apply_vision_sidecar_in(
    backend: &VisionBackend,
    club: &dyn Club,
    msg: &mut ChatMsg,
) -> Result<Option<String>, String> {
    if !should_apply_vision_sidecar_in(backend, club, msg) {
        return Ok(None);
    }
    let images: Vec<Media> = msg
        .attachments
        .iter()
        .filter(|m| matches!(m, Media::Image { .. }))
        .cloned()
        .collect();
    if images.is_empty() {
        return Ok(None);
    }
    let n = images.len();
    let question = sidecar_question(msg.content.trim());
    let (backend, description) = describe_media_in(backend, images, &question)?;
    let body = sidecar_body(&backend, n, &description, msg.content.trim());
    let remaining: Vec<Media> = msg
        .attachments
        .iter()
        .filter(|m| !matches!(m, Media::Image { .. }))
        .cloned()
        .collect();
    msg.content = body.into();
    msg.attachments = remaining.into();
    Ok(Some(format!(
        "vision sidecar via {backend}: described {n} image(s) for text-only {}",
        club.label()
    )))
}

pub(crate) fn drop_images_if_text_only(club: &dyn Club, msg: &mut ChatMsg) {
    if !club_is_text_only_for_vision(club) {
        return;
    }
    if !msg
        .attachments
        .iter()
        .any(|media| matches!(media, Media::Image { .. }))
    {
        return;
    }
    let remaining: Vec<Media> = msg
        .attachments
        .iter()
        .filter(|media| !matches!(media, Media::Image { .. }))
        .cloned()
        .collect();
    let note = format!(
        "{}\n\n{}",
        msg.content.trim(),
        crate::agent::harness::book::d3_roles::pages(
            crate::agent::harness::book::d456_knowledge::VISION,
            [6]
        )
    );
    msg.content = note.into();
    msg.attachments = remaining.into();
}

pub(crate) fn fold_vision_sidecar_into_convo_in(
    backend: &VisionBackend,
    club: &dyn Club,
    convo: &mut [ChatMsg],
) -> Vec<String> {
    let mut notices = Vec::new();
    for msg in convo.iter_mut() {
        if msg.role != ChatRole::User {
            continue;
        }
        match apply_vision_sidecar_in(backend, club, msg) {
            Ok(Some(notice)) => notices.push(notice),
            Ok(None) => {}
            Err(error) => {
                notices.push(format!("vision sidecar failed: {error}"));
                drop_images_if_text_only(club, msg);
            }
        }
    }
    notices
}

pub(crate) fn vision_sidecar_prompt_hint_in(
    backend: &VisionBackend,
    club: &dyn Club,
) -> Option<String> {
    if !backend.configured() {
        return None;
    }
    if !club_is_text_only_for_vision(club) && !vision_sidecar_forced() {
        return None;
    }
    Some(crate::agent::harness::book::n_environment::VISION.cells())
}

fn confined_path(workspace: &Path, raw: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("path must not be empty".to_string());
    }
    let joined = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        workspace.join(raw)
    };
    let mut norm = PathBuf::new();
    for comp in joined.components() {
        match comp {
            std::path::Component::ParentDir => {
                norm.pop();
            }
            std::path::Component::CurDir => {}
            other => norm.push(other.as_os_str()),
        }
    }
    if !norm.starts_with(workspace) {
        return Err(format!("path {raw} escapes the workspace"));
    }
    Ok(norm)
}

fn even_frame_times(dur: f64, n: usize) -> Option<Vec<f64>> {
    if dur <= 0.5 || n == 0 {
        return None;
    }
    let span = (dur - 0.5).max(0.1);
    Some(
        (0..n)
            .map(|i| 0.25 + span * (i as f64 + 0.5) / n as f64)
            .collect(),
    )
}

pub(super) fn probe_duration(path: &Path) -> Option<f64> {
    probe_duration_with_timeout(path, MEDIA_DURATION_PROBE_TIMEOUT)
}

fn probe_duration_with_timeout(path: &Path, timeout: Duration) -> Option<f64> {
    let mut cmd = std::process::Command::new("ffprobe");
    cmd.args([
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "csv=p=0",
        path.to_str()?,
    ]);
    probe_duration_command_with_timeout(cmd, timeout)
}

fn probe_duration_command_with_timeout(
    cmd: std::process::Command,
    timeout: Duration,
) -> Option<f64> {
    let capture = crate::agent::harness::output_timed_fixed_captured(cmd, timeout).ok()?;
    if capture.timed_out
        || capture.cancelled
        || !capture.output_complete()
        || !capture.output.status.success()
    {
        return None;
    }
    String::from_utf8_lossy(&capture.output.stdout)
        .trim()
        .parse()
        .ok()
}

pub(crate) struct VisionLookTool {
    backend: VisionBackend,
    workspace: PathBuf,
    policy: SandboxPolicy,
}

impl VisionLookTool {
    fn in_run(workspace: PathBuf, backend: VisionBackend) -> Self {
        let mut policy = SandboxPolicy::permissive();
        policy.writable_roots.push(workspace.clone());
        Self {
            workspace,
            policy,
            backend,
        }
    }
}

impl Tool for VisionLookTool {
    fn name(&self) -> &str {
        "vision_look"
    }

    fn def(&self) -> ToolDef {
        crate::agent::harness::core_catalog::baseline_def("vision_look")
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let raw = args["path"].as_str().ok_or("missing 'path'")?;
        let path = confined_path(&self.workspace, raw)?;
        if !path.exists() {
            return Err(format!("no such file: {raw}"));
        }
        let question = args["question"]
            .as_str()
            .ok_or("missing 'question'")?
            .trim();
        if question.is_empty() {
            return Err("'question' cannot be empty".to_string());
        }

        let is_image = matches!(
            path.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .as_deref(),
            Some("png") | Some("jpg") | Some("jpeg") | Some("gif") | Some("webp")
        );

        let mut media = Vec::new();
        if is_image {
            media.push(Media::image_from_path(&path)?);
        } else {
            let times: Vec<f64> = if let Some(arr) = args["timestamps"].as_array() {
                arr.iter().filter_map(|v| v.as_f64()).collect()
            } else {
                let n = args["frames"].as_u64().unwrap_or(4).clamp(1, 8) as usize;
                let dur = probe_duration(&path).unwrap_or(0.0);
                even_frame_times(dur, n).ok_or_else(|| {
                    format!("could not determine duration of {raw}; pass explicit timestamps")
                })?
            };
            if times.is_empty() {
                return Err("no frames requested (empty timestamps)".to_string());
            }
            if times.len() > 8 {
                return Err("at most 8 frames per look (context budget)".to_string());
            }
            let path_s = path.to_str().ok_or("non-utf8 path")?;
            for (i, t) in times.iter().enumerate() {
                let out = self.workspace.join(format!(
                    ".angel-vision-frame-{i}-{}.jpg",
                    std::process::id()
                ));
                crate::agent::harness::run_sandboxed(
                    "ffmpeg",
                    &[
                        "-y",
                        "-v",
                        "error",
                        "-ss",
                        &format!("{t:.3}"),
                        "-i",
                        path_s,
                        "-frames:v",
                        "1",
                        "-q:v",
                        "3",
                        out.to_str().ok_or("non-utf8 path")?,
                    ],
                    Some(&self.workspace),
                    &self.policy,
                )?;
                media.push(Media::image_from_path(&out)?);
                let _ = std::fs::remove_file(&out);
            }
        }

        let n = media.len();
        let (backend, text) = describe_media_in(&self.backend, media, question)?;
        Ok(format!(
            "[vision_look via {backend} · {n} frame(s)]\n{text}"
        ))
    }
}

pub(crate) fn maybe_register_vision_tools(
    r: &mut crate::agent::harness::ToolRegistry,
    workspace: PathBuf,
) {
    if !env_flag("ANGEL_VISION_TOOLS", true) {
        return;
    }
    if !r.vision.configured() {
        return;
    }
    r.register(Box::new(VisionLookTool::in_run(
        workspace,
        r.vision.clone(),
    )));
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/vision_catalog_compat.rs"]
mod catalog_compat;
#[cfg(test)]
use catalog_compat::*;

#[cfg(test)]
static DESCRIBE_MEDIA_CALLS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
fn test_vision_club_slot() -> &'static std::sync::Mutex<Option<Arc<dyn Club>>> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Option<Arc<dyn Club>>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(None))
}

#[cfg(test)]
struct LabelOnlyClub {
    label: &'static str,
    modalities: Vec<String>,
}

#[cfg(test)]
impl Club for LabelOnlyClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn label(&self) -> &str {
        self.label
    }
    fn route_metadata(&self) -> crate::agent::club::RouteMetadata {
        crate::agent::club::RouteMetadata {
            input_modalities: self.modalities.clone(),
            ..Default::default()
        }
    }
    fn chat(&self, _messages: &[ChatMsg], _tools: &[ToolDef]) -> Result<ClubReply, String> {
        Ok(ClubReply::Text(String::new()))
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/vision__tests.rs"]
mod tests;
