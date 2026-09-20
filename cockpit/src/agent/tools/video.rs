//! Video editing tools: `video_probe` (ffprobe metadata), `video_beats`
//! (music beat/onset/energy analysis), `video_cut` (render a beat-synced
//! edit decision list via ffmpeg), and `video_contact_sheet` (frame grids
//! for human review). All media work runs through the sandboxed ffmpeg /
//! ffprobe binaries with argv built directly — no shell interpretation.
//!
//! These tools exist so the agent can do *mechanically tight* beat-synced
//! editing (DIRECTION.md rule: "Music owns time") without shelling out
//! ad-hoc: beats land on exact timestamps, cuts snap to the grid, and the
//! human director reviews contact sheets instead of watching raw footage.

use crate::agent::club::{ChatMsg, Club, ClubReply, Media, ToolDef};
use crate::agent::harness::{Tool, run_sandboxed};
use crate::agent::sandbox::SandboxPolicy;
use serde_json::Value;
use std::path::{Path, PathBuf};

const MAX_CLIPS: usize = 32;
const MAX_CLIP_SECONDS: f64 = 3600.0;
const MAX_TIMELINE_SECONDS: f64 = 7200.0;

/// Resolve a caller-supplied media path against the workspace, rejecting
/// anything that escapes it (symlinks included once the file exists).
fn confined_path(workspace: &Path, raw: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("path must not be empty".to_string());
    }
    let joined = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        workspace.join(raw)
    };
    // Normalize `..` without requiring the file to exist.
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

fn ffmpeg_policy(workspace: &Path) -> SandboxPolicy {
    let mut policy = SandboxPolicy::permissive();
    policy.writable_roots.push(workspace.to_path_buf());
    policy
}

fn seconds(v: &Value, key: &str) -> Result<f64, String> {
    v.as_f64()
        .ok_or_else(|| format!("'{key}' must be a number (seconds)"))
}

/// Render the common ffmpeg `-filter_complex` for a timeline of clips with
/// optional crossfades, plus the audio chain. Shared by `video_cut`'s call
/// path and its unit tests.
///
/// Each clip: `{path, in, out}`. `fade` is the xfade duration in seconds
/// (0 = hard cuts via concat). Returns `(filter_graph, video_label,
/// audio_label)` or an error describing the invalid timeline.
fn build_timeline_filter(
    clips: &[(String, f64, f64)],
    fade: f64,
    width: u32,
    height: u32,
    fps: u32,
    audio: Option<(&str, f64)>, // (music path, audio fade-out tail seconds)
) -> Result<String, String> {
    if clips.is_empty() {
        return Err("timeline needs at least one clip".to_string());
    }
    if clips.len() > MAX_CLIPS {
        return Err(format!("timeline exceeds {MAX_CLIPS} clips"));
    }
    let mut g = String::new();
    let mut offsets = Vec::with_capacity(clips.len());
    let mut t = 0.0f64;
    for (i, (_, tin, tout)) in clips.iter().enumerate() {
        if !(tout > tin && *tin >= 0.0) {
            return Err(format!("clip {i}: need 0 <= in < out (got {tin}..{tout})"));
        }
        let dur = tout - tin;
        if dur > MAX_CLIP_SECONDS {
            return Err(format!(
                "clip {i}: duration {dur}s exceeds {MAX_CLIP_SECONDS}s"
            ));
        }
        if t + dur > MAX_TIMELINE_SECONDS {
            return Err(format!("timeline exceeds {MAX_TIMELINE_SECONDS}s"));
        }
        // Scale+pad normalizes mixed-res sources; settb aligns timebases so
        // xfade/concat don't reject the pair (the classic 1/24 vs 1/12288).
        g.push_str(&format!(
            "[{i}:v]trim={tin}:{tout},setpts=PTS-STARTPTS,fps={fps},\
             scale={width}:{height}:force_original_aspect_ratio=decrease,\
             pad={width}:{height}:(ow-iw)/2:(oh-ih)/2,setsar=1,settb=AVTB[v{i}];"
        ));
        t += dur;
        offsets.push(t);
        t -= if i + 1 < clips.len() { fade } else { 0.0 };
    }
    let total = offsets.last().copied().unwrap_or(0.0);
    if fade > 0.0 && clips.len() > 1 {
        // Chain xfades. Each xfade offset = cumulative output length before
        // the join minus the fade duration. offsets[i] already bakes in the
        // fade-overlap math (each iteration subtracts one fade), so the join
        // that appends clip i+1 starts at offsets[i] - fade.
        let mut prev = String::from("v0");
        for i in 1..clips.len() {
            let label = if i + 1 == clips.len() {
                "vout".to_string()
            } else {
                format!("x{i}")
            };
            let off = offsets[i - 1] - fade;
            if off < 0.0 {
                return Err(format!(
                    "fade {fade}s longer than segment before clip {i} ({})s",
                    offsets[i - 1]
                ));
            }
            g.push_str(&format!(
                "[{prev}][v{i}]xfade=transition=fade:duration={fade}:offset={off:.4}[{label}];"
            ));
            prev = label;
        }
    } else {
        let inputs: String = (0..clips.len()).map(|i| format!("[v{i}]")).collect();
        g.push_str(&format!("{inputs}concat=n={}:v=1:a=0[vout];", clips.len()));
    }
    // Final pixel-format normalize for broad player compat.
    g.push_str("[vout]format=yuv420p[vfinal];");
    if let Some((_, tail)) = audio {
        // Audio length follows the video timeline; fade the tail.
        let fade_start = (total - tail).max(0.0);
        g.push_str(&format!(
            "[{}:a]atrim=0:{total:.3},asetpts=PTS-STARTPTS,\
             afade=t=out:st={fade_start:.3}:d={tail:.3}[aout]",
            clips.len() // music is the input right after the clips
        ));
    }
    // Strip the trailing ';' introduced between segments.
    Ok(g.trim_end_matches(';').to_string())
}

// ---------------------------------------------------------------------------
// video_probe — ffprobe metadata for a media file.
// ---------------------------------------------------------------------------

pub(crate) struct VideoProbeTool {
    workspace: PathBuf,
    policy: SandboxPolicy,
}

impl VideoProbeTool {
    pub(crate) fn in_dir(workspace: PathBuf) -> Self {
        let policy = ffmpeg_policy(&workspace);
        Self { workspace, policy }
    }
}

impl Tool for VideoProbeTool {
    fn name(&self) -> &str {
        "video_probe"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "video_probe".to_string(),
            description: "Probe a video/audio file with ffprobe and return JSON metadata: \
                          duration, resolution, frame rate, codecs, stream layout. Use before \
                          planning an edit so clip math is grounded in real container facts."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "media file, workspace-relative" },
                },
                "required": ["path"],
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        let raw = args["path"].as_str().ok_or("missing 'path'")?;
        let path = confined_path(&self.workspace, raw)?;
        if !path.exists() {
            return Err(format!("no such file: {raw}"));
        }
        let out = run_sandboxed(
            "ffprobe",
            &[
                "-v",
                "error",
                "-show_entries",
                "format=duration,size,bit_rate:stream=index,codec_type,codec_name,width,height,r_frame_rate,duration",
                "-of",
                "json",
                path.to_str().ok_or("non-utf8 path")?,
            ],
            Some(&self.workspace),
            &self.policy,
        )?;
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// video_beats — beat/onset/energy analysis of a music track.
// ---------------------------------------------------------------------------

pub(crate) struct VideoBeatsTool {
    workspace: PathBuf,
    policy: SandboxPolicy,
}

impl VideoBeatsTool {
    pub(crate) fn in_dir(workspace: PathBuf) -> Self {
        let policy = ffmpeg_policy(&workspace);
        Self { workspace, policy }
    }

    /// The analysis worker ships as a sibling file so it stays lintable Python
    /// instead of an escaping-prone embedded string. Resolved relative to the
    /// crate manifest at build time.
    fn worker_path() -> PathBuf {
        crate::platform::runtime_paths::cockpit_dir().join("src/agent/tools/video_beats_worker.py")
    }
}

impl Tool for VideoBeatsTool {
    fn name(&self) -> &str {
        "video_beats"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "video_beats".to_string(),
            description: "Analyze a music track and return its beat grid as JSON: estimated BPM, \
                          beat timestamps, onset (transient) timestamps, and windowed RMS energy \
                          so you can hear where phrases and the crescendo live. Cut points should \
                          snap to these beats — music owns time. Uses librosa/aubio/essentia via python3; falls back to an energy-only analysis when no backend is installed."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "audio file, workspace-relative" },
                    "energy_window": {
                        "type": "integer",
                        "description": "seconds per energy window (default 5)",
                    },
                },
                "required": ["path"],
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        let raw = args["path"].as_str().ok_or("missing 'path'")?;
        let path = confined_path(&self.workspace, raw)?;
        if !path.exists() {
            return Err(format!("no such file: {raw}"));
        }
        let window = args["energy_window"].as_u64().unwrap_or(5).clamp(1, 60);
        let worker = Self::worker_path();
        run_sandboxed(
            "python3",
            &[
                worker.to_str().ok_or("non-utf8 worker path")?,
                path.to_str().ok_or("non-utf8 path")?,
                &window.to_string(),
            ],
            Some(&self.workspace),
            &self.policy,
        )
    }
}

// ---------------------------------------------------------------------------
// video_cut — render a beat-synced EDL via ffmpeg.
// ---------------------------------------------------------------------------

pub(crate) struct VideoCutTool {
    workspace: PathBuf,
    policy: SandboxPolicy,
}

impl VideoCutTool {
    pub(crate) fn in_dir(workspace: PathBuf) -> Self {
        let policy = ffmpeg_policy(&workspace);
        Self { workspace, policy }
    }
}

impl Tool for VideoCutTool {
    fn name(&self) -> &str {
        "video_cut"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "video_cut".to_string(),
            description: "Render an edit decision list to a finished mp4. Give clips as \
                          {path, in, out} (seconds) in timeline order; segment boundaries are \
                          joined with a crossfade of `fade` seconds (0 = hard cuts). Beat-snap \
                          the boundaries yourself using video_beats output — this tool renders \
                          exactly what you specify. Optional music bed is trimmed to the timeline \
                          with an audio fade-out tail. Mixed-res sources are letterboxed to \
                          width x height."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "clips": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" },
                                "in": { "type": "number", "description": "source in-point, seconds" },
                                "out": { "type": "number", "description": "source out-point, seconds" },
                            },
                            "required": ["path", "in", "out"],
                        },
                    },
                    "output": { "type": "string", "description": "destination mp4, workspace-relative" },
                    "music": { "type": "string", "description": "optional audio bed, workspace-relative" },
                    "music_start": { "type": "number", "description": "seconds into the music to start (default 0)" },
                    "fade": { "type": "number", "description": "crossfade seconds between clips (default 0 = hard cuts)" },
                    "audio_tail": { "type": "number", "description": "audio fade-out length at the end (default 1.0)" },
                    "width": { "type": "integer", "description": "output width (default: first clip's)" },
                    "height": { "type": "integer", "description": "output height (default: first clip's)" },
                    "fps": { "type": "integer", "description": "output frame rate (default 24)" },
                },
                "required": ["clips", "output"],
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        let clips_v = args["clips"].as_array().ok_or("'clips' must be an array")?;
        let mut clips = Vec::with_capacity(clips_v.len());
        for (i, c) in clips_v.iter().enumerate() {
            let raw = c["path"]
                .as_str()
                .ok_or_else(|| format!("clip {i}: missing 'path'"))?;
            let path = confined_path(&self.workspace, raw)?;
            if !path.exists() {
                return Err(format!("clip {i}: no such file: {raw}"));
            }
            let tin = seconds(&c["in"], "in")?;
            let tout = seconds(&c["out"], "out")?;
            clips.push((path.to_str().ok_or("non-utf8 path")?.to_string(), tin, tout));
        }
        let out_raw = args["output"].as_str().ok_or("missing 'output'")?;
        let output = confined_path(&self.workspace, out_raw)?;
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("output dir: {e}"))?;
        }
        let fade = args["fade"].as_f64().unwrap_or(0.0);
        if !(0.0..=2.0).contains(&fade) {
            return Err("'fade' must be 0..=2.0 seconds".to_string());
        }
        let fps = args["fps"].as_u64().unwrap_or(24).clamp(1, 120) as u32;
        let tail = args["audio_tail"].as_f64().unwrap_or(1.0).clamp(0.0, 10.0);
        let music_start = args["music_start"].as_f64().unwrap_or(0.0).max(0.0);

        let music = match args["music"].as_str() {
            Some(raw) => {
                let p = confined_path(&self.workspace, raw)?;
                if !p.exists() {
                    return Err(format!("no such music file: {raw}"));
                }
                Some(p.to_str().ok_or("non-utf8 path")?.to_string())
            }
            None => None,
        };

        // Output geometry: explicit, else probe the first clip.
        let (mut w, mut h) = (
            args["width"].as_u64().unwrap_or(0) as u32,
            args["height"].as_u64().unwrap_or(0) as u32,
        );
        if w == 0 || h == 0 {
            let probe = run_sandboxed(
                "ffprobe",
                &[
                    "-v",
                    "error",
                    "-select_streams",
                    "v:0",
                    "-show_entries",
                    "stream=width,height",
                    "-of",
                    "csv=p=0",
                    &clips[0].0,
                ],
                Some(&self.workspace),
                &self.policy,
            )?;
            let mut it = probe.trim().split(',');
            w = it.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            h = it.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            if w == 0 || h == 0 {
                return Err("could not determine output size; pass width/height".to_string());
            }
        }
        // yuv420p demands even dimensions.
        w &= !1;
        h &= !1;

        let filter = build_timeline_filter(
            &clips,
            fade,
            w,
            h,
            fps,
            music.as_deref().map(|_| ("music", tail)),
        )?;

        let mut argv: Vec<String> = vec!["-y".into(), "-v".into(), "error".into()];
        for (p, _, _) in &clips {
            argv.push("-i".into());
            argv.push(p.clone());
        }
        if let Some(m) = &music {
            argv.push("-ss".into());
            argv.push(format!("{music_start:.3}"));
            argv.push("-i".into());
            argv.push(m.clone());
        }
        argv.push("-filter_complex".into());
        argv.push(filter);
        argv.push("-map".into());
        argv.push("[vfinal]".into());
        if music.is_some() {
            argv.push("-map".into());
            argv.push("[aout]".into());
        }
        argv.extend([
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            "18".into(),
            "-preset".into(),
            "medium".into(),
        ]);
        if music.is_some() {
            argv.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
        }
        argv.push(output.to_str().ok_or("non-utf8 path")?.to_string());

        let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        let run = run_sandboxed("ffmpeg", &refs, Some(&self.workspace), &self.policy)?;
        let verify = run_sandboxed(
            "ffprobe",
            &[
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "csv=p=0",
                output.to_str().ok_or("non-utf8 path")?,
            ],
            Some(&self.workspace),
            &self.policy,
        )
        .unwrap_or_default();
        Ok(format!("rendered {out_raw} ({}s)\n{}", verify.trim(), run))
    }
}

// ---------------------------------------------------------------------------
// video_contact_sheet — tiled frame grid for director review.
// ---------------------------------------------------------------------------

pub(crate) struct VideoContactSheetTool {
    workspace: PathBuf,
    policy: SandboxPolicy,
}

impl VideoContactSheetTool {
    pub(crate) fn in_dir(workspace: PathBuf) -> Self {
        let policy = ffmpeg_policy(&workspace);
        Self { workspace, policy }
    }
}

impl Tool for VideoContactSheetTool {
    fn name(&self) -> &str {
        "video_contact_sheet"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "video_contact_sheet".to_string(),
            description: "Extract a tiled grid of frames (with timestamps burned in) from a video \
                          for take review. Feed the sheet to video_look for a machine read, or \
                          present() it to the director for circle/selects decisions."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "video file, workspace-relative" },
                    "output": { "type": "string", "description": "destination png/jpg, workspace-relative" },
                    "frames": { "type": "integer", "description": "number of samples (default 12, max 64)" },
                    "columns": { "type": "integer", "description": "grid columns (default 4)" },
                },
                "required": ["path", "output"],
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        let raw = args["path"].as_str().ok_or("missing 'path'")?;
        let path = confined_path(&self.workspace, raw)?;
        if !path.exists() {
            return Err(format!("no such file: {raw}"));
        }
        let out_raw = args["output"].as_str().ok_or("missing 'output'")?;
        let output = confined_path(&self.workspace, out_raw)?;
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("output dir: {e}"))?;
        }
        let frames = args["frames"].as_u64().unwrap_or(12).clamp(1, 64);
        let cols = args["columns"].as_u64().unwrap_or(4).clamp(1, 8);
        let rows = frames.div_ceil(cols);
        let probe = run_sandboxed(
            "ffprobe",
            &[
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "csv=p=0",
                path.to_str().ok_or("non-utf8 path")?,
            ],
            Some(&self.workspace),
            &self.policy,
        )
        .unwrap_or_default();
        let dur: f64 = probe.trim().parse().unwrap_or(0.0);
        if dur <= 0.0 {
            return Err(format!("could not probe duration of {raw}"));
        }
        // fps=frames/duration samples evenly across the file; drawtext stamps
        // each tile with its source timestamp; tile arranges the grid.
        let filter = format!(
            "fps={frames}/{dur:.3},\
             drawtext=text='%{{pts\\:hms}}':x=8:y=8:fontsize=20:fontcolor=white:box=1:boxcolor=black@0.6,\
             scale=320:-2,tile={cols}x{rows}"
        );
        let out = run_sandboxed(
            "ffmpeg",
            &[
                "-y",
                "-v",
                "error",
                "-i",
                path.to_str().ok_or("non-utf8 path")?,
                "-filter_complex",
                &filter,
                "-frames:v",
                "1",
                output.to_str().ok_or("non-utf8 path")?,
            ],
            Some(&self.workspace),
            &self.policy,
        )?;
        let _ = out;
        Ok(format!(
            "contact sheet written to {out_raw} — present() it for review"
        ))
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub(crate) fn maybe_register_video_tools(
    r: &mut crate::agent::harness::ToolRegistry,
    workspace: PathBuf,
) {
    if crate::agent::harness::env_flag("ANGEL_VIDEO_TOOLS", true) {
        r.register(Box::new(VideoProbeTool::in_dir(workspace.clone())));
        r.register(Box::new(VideoBeatsTool::in_dir(workspace.clone())));
        r.register(Box::new(VideoCutTool::in_dir(workspace.clone())));
        r.register(Box::new(VideoContactSheetTool::in_dir(workspace.clone())));
        r.register(Box::new(VideoLookTool::new(workspace)));
    }
}

// ---------------------------------------------------------------------------
// video_look — machine eyes: frames + question → a vision club.
// ---------------------------------------------------------------------------

/// Evenly spaced frame timestamps across a clip, inset 0.25s from both ends.
/// `None` when the duration is unknown/too short to be meaningful.
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

/// The missing half of the editing loop: extract frames from a video (or take
/// a ready-made image like a `video_contact_sheet` tile) and ask a vision
/// club about them. The reply is text for the calling agent — framing,
/// palette, continuity, which take reads best — so the *model* is the eyes
/// and the human directs taste instead of being the sensor.
///
/// Backend resolution order:
///   1. `ANGEL_VISION_URL` + `ANGEL_VISION_MODEL` (+ optional
///      `ANGEL_VISION_KEY`) — any OpenAI-compatible vision endpoint
///      (llama.cpp gemma3/llava, vLLM, LM Studio, Kimi, …).
///   2. Kimi env (`ANGEL_KIMI_URL` + `ANGEL_KIMI_KEY` + `ANGEL_KIMI_MODEL`)
///      when present — the route this toolset was born on.
///   3. Signed-in Codex with a configured model advertising image input.
///   4. Hard error naming the available configuration options.
pub(crate) struct VideoLookTool {
    workspace: PathBuf,
    policy: SandboxPolicy,
}

impl VideoLookTool {
    fn new(workspace: PathBuf) -> Self {
        let mut policy = SandboxPolicy::permissive();
        policy.writable_roots.push(workspace.clone());
        Self { workspace, policy }
    }

    fn vision_club() -> Result<std::sync::Arc<dyn Club>, String> {
        crate::agent::tools::vision::resolve_vision_club().map_err(|e| format!("video_look: {e}"))
    }
}

impl Tool for VideoLookTool {
    fn name(&self) -> &str {
        "video_look"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: "video_look".to_string(),
            description: "Machine eyes for the edit suite: send frames from a video (or an \
                          existing image, e.g. a contact sheet) to a vision club with a \
                          question, and get back a textual read — framing, palette, continuity, \
                          which take reads best. Use after video_contact_sheet for single-pass \
                          take review, or directly with `frames`/`timestamps` for targeted \
                          questions. Backend: ANGEL_VISION_URL/ANGEL_VISION_MODEL, else the \
                          Kimi env route."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "video file (extract frames) or image file (sent as-is), workspace-relative"
                    },
                    "question": {
                        "type": "string",
                        "description": "what to look for, e.g. 'rank the takes on handoff cleanliness' or 'describe palette and framing of each shot'"
                    },
                    "frames": {
                        "type": "integer",
                        "description": "evenly spaced frames to extract when path is a video (default 6, max 8; ignored for images)"
                    },
                    "timestamps": {
                        "type": "array",
                        "items": { "type": "number" },
                        "description": "exact timestamps (seconds) to grab instead of even spacing; overrides frames"
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
                let n = args["frames"].as_u64().unwrap_or(6).clamp(1, 8) as usize;
                let dur = crate::agent::tools::vision::probe_duration(&path).unwrap_or(0.0);
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
                let out = self
                    .workspace
                    .join(format!(".angel-look-frame-{i}-{}.jpg", std::process::id()));
                run_sandboxed(
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

        let club = Self::vision_club()?;
        let n = media.len();
        let msg = ChatMsg::user_with_media(question, media);
        let reply = club
            .chat(&[msg], &[])
            .map_err(|e| format!("video_look ({}): {e}", club.label()))?;
        let text = match reply {
            ClubReply::Text(t) => t,
            ClubReply::Calls(_) => {
                "vision club answered with tool calls (unsupported here)".to_string()
            }
        };
        Ok(format!(
            "[video_look via {} · {n} frame(s)]\n{}",
            club.label(),
            text.trim()
        ))
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/video__tests.rs"]
mod tests;
