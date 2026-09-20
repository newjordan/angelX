//! Vision sidecar for text-only clubs (DeepSeek V4 Flash and friends).
//!
//! Community pattern (X, 2026-08): DeepSeek Flash has no image input, so
//! people bolt a small OpenAI-compatible VLM in front — describe frames to
//! text, then let the main model reason. Qwen-MM-Plugins is the popular full
//! plugin stack; the lightweight fit for angel is the same idea we already
//! use for `video_look`: `ANGEL_VISION_URL` + `ANGEL_VISION_MODEL` (+ key),
//! with Kimi or the signed-in Codex image model as fallback routes.
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

/// Whether an explicit backend or a signed-in, image-capable Codex route exists.
pub(crate) fn vision_backend_configured() -> bool {
    let explicit =
        std::env::var("ANGEL_VISION_URL").is_ok() && std::env::var("ANGEL_VISION_MODEL").is_ok();
    let kimi = std::env::var("ANGEL_KIMI_URL").is_ok() && std::env::var("ANGEL_KIMI_KEY").is_ok();
    explicit || kimi || codex_vision_config().is_ok()
}

fn codex_vision_config() -> Result<
    (
        crate::agent::openai_codex::ChatGptAuth,
        crate::agent::club::codex_selection::Selection,
        crate::agent::openai_codex::CodexModelInfo,
    ),
    String,
> {
    use crate::agent::openai_codex::{ChatGptAuth, CodexClub};
    let auth = ChatGptAuth::load().ok_or("no signed-in Codex account")?;
    let selection = CodexClub::resolve_selection();
    if let Some(error) = &selection.error {
        return Err(error.clone());
    }
    let spec = CodexClub::model_catalog()
        .into_iter()
        .find(|candidate| candidate.slug == selection.model)
        .filter(|candidate| {
            candidate
                .input_modalities
                .iter()
                .any(|m| m.eq_ignore_ascii_case("image"))
        })
        .ok_or_else(|| {
            format!(
                "Codex model {} has no cached image capability",
                selection.model
            )
        })?;
    Ok((auth, selection, spec))
}

/// OpenAI-compatible vision club used by `vision_look`, `video_look`, and the
/// auto `/see` sidecar. Resolution order:
///   1. `ANGEL_VISION_URL` + `ANGEL_VISION_MODEL` (+ optional `ANGEL_VISION_KEY`)
///   2. Kimi (`ANGEL_KIMI_URL` + `ANGEL_KIMI_KEY` + optional model)
///   3. Signed-in Codex, preserving its configured model and reasoning effort,
///      only when the model catalog explicitly advertises image input.
pub(crate) fn resolve_vision_club() -> Result<Arc<dyn Club>, String> {
    if let (Some(url), Some(model)) = (
        std::env::var("ANGEL_VISION_URL").ok(),
        std::env::var("ANGEL_VISION_MODEL").ok(),
    ) {
        let key = std::env::var("ANGEL_VISION_KEY").ok();
        return Ok(Arc::new(HttpClub::new("vision", url, model, key)));
    }
    if let (Ok(url), Ok(key)) = (
        std::env::var("ANGEL_KIMI_URL"),
        std::env::var("ANGEL_KIMI_KEY"),
    ) {
        let model = std::env::var("ANGEL_KIMI_MODEL").unwrap_or_else(|_| "k3-256k".into());
        return Ok(Arc::new(HttpClub::new(
            "kimi-vision",
            url,
            model,
            Some(key),
        )));
    }
    let (auth, selection, spec) = codex_vision_config().map_err(|error| {
        format!(
            "no vision backend configured — set ANGEL_VISION_URL + ANGEL_VISION_MODEL \
         (+ ANGEL_VISION_KEY), or ANGEL_KIMI_URL + ANGEL_KIMI_KEY, or sign in to \
         Codex with an image-capable model ({error})"
        )
    })?;
    let levels = spec
        .supported_reasoning_levels
        .iter()
        .map(|level| level.effort.clone())
        .collect();
    let club = crate::agent::openai_codex::CodexClub::new_with_route_metadata_shared(
        format!("codex-vision/{}", selection.model),
        selection.model.clone(),
        crate::agent::openai_codex::CodexClub::shared_state(auth),
        Some(selection.effort.clone()),
        levels,
        spec.route_metadata(),
    )
    .with_selection(selection);
    Ok(Arc::new(club))
}

/// Sidecar arming: default on when a vision backend is configured.
/// `ANGEL_VISION_SIDECAR=0` disables auto `/see` rewrite (tools stay available).
pub(crate) fn vision_sidecar_enabled() -> bool {
    env_flag("ANGEL_VISION_SIDECAR", true) && vision_backend_configured()
}

/// True when the in-hand club cannot take image parts natively.
///
/// Declared capability wins. A club that publishes
/// `RouteMetadata.input_modalities` — the HTTP static model map, the Codex
/// catalog, the Grok route — is believed: natively multimodal models keep
/// their attachments and never pay a sidecar hop. Only clubs that declare
/// *nothing* fall back to the name heuristic, where known vision fragments
/// short-circuit to false and the text-only families (local/fleet DeepSeek V4
/// serves, V4 Pro) are caught. A current cloud V4.1 Flash is `deepseek-flash`
/// and is natively multimodal, so neither list may claim the live cloud id: the
/// configured DeepSeek route states its own capability, and the retired
/// `deepseek-v4-flash` fallback only covers an undeclared serve that still uses
/// the old checkpoint name.
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
        "claude-3",
        "claude-4",
        "gemini",
        "pixtral",
        "qwen2.5-vl",
        "qwen2-vl",
        "qwen-vl",
        "kimi-vl",
    ]
    .iter()
    .any(|needle| hay.contains(needle))
}

/// Fallback text-only families, consulted only when a club declares no
/// `input_modalities` — a declared route (the configured DeepSeek seat, the
/// Codex catalog, the Grok route) always wins, so nothing here can override a
/// provider's own capability statement.
fn text_only_name(hay: &str) -> bool {
    [
        // Spark-local / fleet DeepSeek V4 serves and the local route's own
        // identity markers.
        "dsflash",
        "dspark",
        "deepseek-flash-local",
        // The retired V4 Flash id: an undeclared route serving it is presumed
        // the older text-only checkpoint (a local/fleet serve reusing the old
        // name), never the cloud model. The configured DeepSeek route declares
        // `text+image` and never reaches this fallback.
        //
        // The live cloud id (`deepseek-flash`) is absent on purpose: matching it
        // by name is what routed screenshots through the sidecar (2026-09-17
        // repair). Cloud V4 Pro and the retired chat/reasoner ids stay
        // text-only.
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

/// Force sidecar even when the club looks multimodal (`ANGEL_VISION_SIDECAR=force`).
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

/// Should we rewrite this user message's image attachments into text?
pub(crate) fn should_apply_vision_sidecar(club: &dyn Club, msg: &ChatMsg) -> bool {
    if !vision_sidecar_enabled() && !vision_sidecar_forced() {
        return false;
    }
    if !vision_backend_configured() {
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

/// Ask the vision club about pre-encoded media. Pure network side-effect.
/// Callers must not run this on the UI/draw thread — `club.chat` blocks.
pub(crate) fn describe_media(
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
    let club = vision_describe_club()?;
    let msg = ChatMsg::user_with_media(question, media);
    let reply = club
        .chat(&[msg], &[])
        .map_err(|e| format!("vision sidecar ({}): {e}", club.label()))?;
    let text = match reply {
        ClubReply::Text(t) => t,
        ClubReply::Calls(_) => {
            "vision club answered with tool calls (unsupported for sidecar)".to_string()
        }
    };
    Ok((club.label().to_string(), text.trim().to_string()))
}

fn vision_describe_club() -> Result<Arc<dyn Club>, String> {
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
    resolve_vision_club()
}

/// One image request, without starting a cockpit session or an agent tool loop.
/// Useful to running sessions whose binary predates automatic vision routing.
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
    let (backend, description) =
        describe_media(vec![media], &question).map_err(std::io::Error::other)?;
    println!("[vision_look via {backend} · 1 image]\n{description}");
    Ok(())
}

/// Rewrite a user message that carries images into text-only form the main
/// (text) club can consume. Returns a short operator-facing notice on success.
///
/// On failure leaves `msg` unchanged and returns `Err` so the caller can surface
/// the backend problem instead of silently sending bare base64 to DeepSeek.
pub(crate) fn apply_vision_sidecar(
    club: &dyn Club,
    msg: &mut ChatMsg,
) -> Result<Option<String>, String> {
    if !should_apply_vision_sidecar(club, msg) {
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
    let question = if msg.content.trim().is_empty() {
        "Describe this image in detail. Include any visible text (OCR), layout, \
         objects, and anything needed to answer a follow-up question about it."
            .to_string()
    } else {
        format!(
            "The operator's question about this image:\n{}\n\n\
             Describe the image carefully (OCR any text) so a text-only model \
             can answer that question without seeing the pixels.",
            msg.content.trim()
        )
    };
    let (backend, description) = describe_media(images, &question)?;
    let original = msg.content.trim().to_string();
    let mut body = String::new();
    body.push_str("[vision sidecar · ");
    body.push_str(&backend);
    body.push_str(" · ");
    body.push_str(&n.to_string());
    body.push_str(" image(s)]\n");
    body.push_str(&description);
    if !original.is_empty() {
        body.push_str("\n\nOperator question: ");
        body.push_str(&original);
    }
    // Drop image parts so a text-only endpoint never sees image_url (400).
    // Keep non-image attachments (audio) if any.
    let remaining: Vec<Media> = msg
        .attachments
        .iter()
        .filter(|m| !matches!(m, Media::Image { .. }))
        .cloned()
        .collect();
    *msg = if remaining.is_empty() {
        ChatMsg::user(body)
    } else {
        ChatMsg::user_with_media(body, remaining)
    };
    Ok(Some(format!(
        "vision sidecar via {backend}: described {n} image(s) for text-only {}",
        club.label()
    )))
}

/// Strip image parts from a text-only driver so a failed sidecar cannot 400
/// the first provider hop. Leaves attachments in place when the club can take
/// them natively.
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
        "{}\n\n[vision sidecar unavailable — image attachments dropped; \
         configure ANGEL_VISION_URL + ANGEL_VISION_MODEL or use vision_look \
         once a backend is up]",
        msg.content.trim()
    );
    *msg = if remaining.is_empty() {
        ChatMsg::user(note)
    } else {
        ChatMsg::user_with_media(note, remaining)
    };
}

/// Rewrite image-bearing user turns in `convo` before hop 1. Cheap no-op when
/// `should_apply_vision_sidecar` is false (no images / sidecar off / no backend).
/// Must run on the agent-turn worker so the UI thread never blocks on `club.chat`.
pub(crate) fn fold_vision_sidecar_into_convo(
    club: &dyn Club,
    convo: &mut [ChatMsg],
) -> Vec<String> {
    let mut notices = Vec::new();
    for msg in convo.iter_mut() {
        if msg.role != ChatRole::User {
            continue;
        }
        match apply_vision_sidecar(club, msg) {
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

/// Compact system/tool hint when the sidecar is armed for a text-only driver.
pub(crate) fn vision_sidecar_prompt_hint(club: &dyn Club) -> Option<String> {
    if !vision_backend_configured() {
        return None;
    }
    if !club_is_text_only_for_vision(club) && !vision_sidecar_forced() {
        return None;
    }
    Some(
        "Vision: this driver is text-only. Images attached with /see are auto-described \
         by the vision sidecar (ANGEL_VISION_*). For paths on disk, call vision_look \
         (or video_look) with path + question — do not invent visual details."
            .to_string(),
    )
}

// ---------------------------------------------------------------------------
// vision_look — still images (and videos via ffmpeg frames) for the agent
// ---------------------------------------------------------------------------

/// Workspace-confined path helper (same rules as video tools).
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
    // Duration discovery sits synchronously inside an agent-facing tool call.
    // A malformed clip or wedged ffprobe must not own the turn forever, even
    // under YOLO. The result is a tiny scalar, so overflow or any incomplete
    // capture fails closed and callers retain their explicit-timestamp escape.
    let capture = crate::agent::harness::output_timed_fixed_captured(cmd, timeout).ok()?;
    if capture.timed_out
        || capture.cancelled
        || capture.stdout_truncated
        || capture.stderr_truncated
        || !capture.output.status.success()
    {
        return None;
    }
    String::from_utf8_lossy(&capture.output.stdout)
        .trim()
        .parse()
        .ok()
}

/// Agent-facing eyes for text-only clubs: look at a workspace image (or sample
/// frames from a video) via the vision sidecar backend.
pub(crate) struct VisionLookTool {
    workspace: PathBuf,
    policy: SandboxPolicy,
}

impl VisionLookTool {
    pub(crate) fn new(workspace: PathBuf) -> Self {
        let mut policy = SandboxPolicy::permissive();
        policy.writable_roots.push(workspace.clone());
        Self { workspace, policy }
    }
}

impl Tool for VisionLookTool {
    fn name(&self) -> &str {
        "vision_look"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "vision_look".to_string(),
            description: "Machine eyes for a text-only driver (DeepSeek Flash, …): send a \
                          workspace image (png/jpg/webp/gif) or sample frames from a video to \
                          the vision sidecar and get a textual read — OCR, layout, objects, \
                          UI state. Prefer this over inventing visual details. Backend: \
                          ANGEL_VISION_URL/ANGEL_VISION_MODEL (OpenAI-compatible VLM), else Kimi, \
                          else the signed-in Codex image model. \
                          /see on a text-only club auto-routes through the same sidecar."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "image or video path, workspace-relative"
                    },
                    "question": {
                        "type": "string",
                        "description": "what to look for, e.g. 'OCR all text' or 'describe the UI state'"
                    },
                    "frames": {
                        "type": "integer",
                        "description": "evenly spaced frames when path is a video (default 4, max 8; ignored for images)"
                    },
                    "timestamps": {
                        "type": "array",
                        "items": { "type": "number" },
                        "description": "exact timestamps (seconds) for video frames; overrides frames"
                    }
                },
                "required": ["path", "question"],
            }),
        }
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
        let (backend, text) = describe_media(media, question)?;
        Ok(format!(
            "[vision_look via {backend} · {n} frame(s)]\n{text}"
        ))
    }
}

/// Register `vision_look` whenever a vision backend is configured.
/// Independent of `ANGEL_VIDEO_TOOLS` so DeepSeek sessions get eyes without
/// the full edit suite.
pub(crate) fn maybe_register_vision_tools(
    r: &mut crate::agent::harness::ToolRegistry,
    workspace: PathBuf,
) {
    // Default on when backend is set; ANGEL_VISION_TOOLS=0 opts out.
    if !env_flag("ANGEL_VISION_TOOLS", true) {
        return;
    }
    if !vision_backend_configured() {
        return;
    }
    r.register(Box::new(VisionLookTool::new(workspace)));
}

#[cfg(test)]
static DESCRIBE_MEDIA_CALLS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
fn test_vision_club_slot() -> &'static std::sync::Mutex<Option<Arc<dyn Club>>> {
    static SLOT: std::sync::OnceLock<std::sync::Mutex<Option<Arc<dyn Club>>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::Mutex::new(None))
}

/// Dummy club used only in pure unit tests of sidecar gating.
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
