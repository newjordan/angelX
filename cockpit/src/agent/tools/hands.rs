//! `desktop` + `browser` — angel's hands on Omarchy (Hyprland/Wayland).
//!
//! Observe actions (status, windows, screenshots, page snapshots) run free.
//! Acts (focus, key, type, click, navigate, submit-shaped clicks) are
//! classified as action-capsule operations in `harness::action_capsule`, so
//! the operator approves each bounded batch — or skips the modal under the
//! existing YOLO profiles (`/yolo`, `/yolos`). `ANGEL_HANDS=0` hides both.
//!
//! Desktop backends are the Omarchy-native CLIs: `hyprctl`, `grim`, `wtype`,
//! `wl-copy`; mouse clicks use `ydotool`/`wlrctl` when present and fail with
//! an install hint when not. The browser backend drives the system Chromium
//! over CDP from a zero-dependency Node script (`browser_driver.mjs`, global
//! `WebSocket` + `fetch`), spawned outside the tool sandbox like MCP servers
//! because it must reach the operator's real session.

use crate::agent::club::ToolDef;
use crate::agent::harness::{env_flag, Tool, ToolRegistry};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) const MAX_TYPE_CHARS: usize = 4_000;
const CMD_TIMEOUT: Duration = Duration::from_secs(15);
const DRIVER_TIMEOUT: Duration = Duration::from_secs(45);

/// True when `(tool, action)` changes the world and needs an action capsule.
/// Unknown actions fail closed (treated as acts).
pub(crate) fn is_act(tool: &str, action: &str) -> bool {
    !matches!(
        (tool, action),
        ("desktop", "status" | "windows" | "screenshot" | "cursor" | "ground")
            | ("browser", "status" | "snapshot" | "screenshot")
    )
}

pub(crate) fn validate_browser_url(url: &str) -> Result<String, String> {
    let url = url.trim();
    if url == "about:blank" {
        return Ok(url.to_string());
    }
    let parsed = url::Url::parse(url).map_err(|e| format!("bad url {url:?}: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(format!(
            "browser only opens http(s) pages or about:blank, not {url:?}"
        ));
    }
    Ok(url.to_string())
}

pub(crate) fn validate_type_text(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Err("'text' must not be empty".to_string());
    }
    if text.chars().count() > MAX_TYPE_CHARS {
        return Err(format!("'text' exceeds {MAX_TYPE_CHARS} chars"));
    }
    Ok(())
}

/// Key chords are `+`-joined keysyms (`ctrl+l`, `alt+Tab`, `Return`). Only
/// keysym characters survive, so a chord can never become shell syntax.
pub(crate) fn validate_key(key: &str) -> Result<Vec<String>, String> {
    let parts: Vec<String> = key.split('+').map(|p| p.trim().to_string()).collect();
    if parts.is_empty() || parts.iter().any(|p| p.is_empty()) {
        return Err("'key' must be a keysym or chord like 'ctrl+l'".to_string());
    }
    for p in &parts {
        if !p
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        {
            return Err(format!("invalid keysym {p:?}"));
        }
    }
    Ok(parts)
}

pub(crate) fn validate_coords(x: i64, y: i64) -> Result<(), String> {
    if !(0..16_384).contains(&x) || !(0..16_384).contains(&y) {
        return Err(format!(
            "click coords ({x},{y}) outside any plausible display"
        ));
    }
    Ok(())
}

/// The GUI grounding model's trained system prompt, verbatim from
/// microsoft/Mage-VL-Cowork `osworld_eval/mm_agents/mage_gui_agent.py`
/// (`MAGE_GUI_SYSTEM_PROMPT`, Apache-2.0).
const GROUND_SYSTEM: &str = include_str!("hands_ground_prompt.txt");
const GROUND_TIMEOUT: Duration = Duration::from_secs(180);

/// The first-step user turn the grounding model was trained on (the
/// reference agent's), with the target pinned as on-screen so it points
/// instead of planning a search.
pub(crate) fn ground_user_text(target: &str) -> String {
    let target = target.trim().trim_end_matches('.');
    let target = match target.get(..6) {
        Some(head) if head.eq_ignore_ascii_case("click ") => target[6..].trim_start(),
        _ => target,
    };
    format!(
        "\nPlease generate the next move according to the UI screenshot, \
         instruction and previous actions.\n\nInstruction: Click {target}. \
         The target is visible on the current screen; click it directly in one \
         action.\n\nPrevious actions:\nNone"
    )
}

/// The model's point from its reply: the first `<tool_call>` whose
/// `arguments.coordinate` is two numbers in its `[0, 999]` frame. Anything
/// else fails closed — a garbled reply is an error, never a guessed click.
pub(crate) fn parse_ground_reply(reply: &str) -> Result<(f64, f64), String> {
    let mut rest = reply;
    while let Some(start) = rest.find("<tool_call>") {
        let body = &rest[start + "<tool_call>".len()..];
        let end = body.find("</tool_call>").unwrap_or(body.len());
        rest = &body[end..];
        let Ok(call) = serde_json::from_str::<Value>(body[..end].trim()) else {
            continue;
        };
        let Some(coord) = call
            .pointer("/arguments/coordinate")
            .and_then(Value::as_array)
        else {
            continue;
        };
        let (Some(x), Some(y)) = (
            coord.first().and_then(Value::as_f64),
            coord.get(1).and_then(Value::as_f64),
        ) else {
            continue;
        };
        if coord.len() != 2 || !(0.0..=999.0).contains(&x) || !(0.0..=999.0).contains(&y) {
            return Err(format!("ground model pointed off-screen: {coord:?}"));
        }
        return Ok((x, y));
    }
    let snippet: String = reply.chars().take(200).collect();
    Err(format!("ground model gave no point: {snippet:?}"))
}

/// Zoom cascade after the first look: each pass re-grounds on a crop 1/z of
/// the frame around the last point, scaled back up to the full frame. The
/// model reads its `[0, 999]` frame at the screen size it trained on, so a
/// bare crop halves its answers; scaled up, small targets grow instead.
/// Picked on the 2026-10-30 probe (web 18 → 32 of 38 hits, median miss
/// 25 → 5 px; desktop 11 → 17 of 29).
pub(crate) const GROUND_ZOOMS: [u32; 2] = [2, 3];

/// The model's `[0, 999]` point on a `size` image, in that image's pixels
/// (the reference agent's `x / 999 · (W − 1)`).
pub(crate) fn frame_to_px(point: (f64, f64), size: (u32, u32)) -> (f64, f64) {
    (
        point.0 / 999.0 * (f64::from(size.0) - 1.0),
        point.1 / 999.0 * (f64::from(size.1) - 1.0),
    )
}

/// The crop `(x0, y0, w, h)` 1/`zoom` of `shot`, centred on `at` and kept
/// inside the frame.
pub(crate) fn zoom_window(at: (f64, f64), shot: (u32, u32), zoom: u32) -> (u32, u32, u32, u32) {
    let zoom = zoom.max(1);
    let (w, h) = ((shot.0 / zoom).max(1), (shot.1 / zoom).max(1));
    let x0 = (at.0 - f64::from(w) / 2.0).round().clamp(0.0, f64::from(shot.0 - w)) as u32;
    let y0 = (at.1 - f64::from(h) / 2.0).round().clamp(0.0, f64::from(shot.1 - h)) as u32;
    (x0, y0, w, h)
}

/// A screenshot pixel in Hyprland layout coordinates for `click`: the
/// monitor's origin plus the pixel offset over its scale.
pub(crate) fn px_to_layout(px: (f64, f64), origin: (i64, i64), scale: f64) -> (i64, i64) {
    let scale = if scale > 0.0 { scale } else { 1.0 };
    (
        origin.0 + (px.0 / scale).round() as i64,
        origin.1 + (px.1 / scale).round() as i64,
    )
}

/// The model's `[0, 999]` point on a whole-output screenshot, in layout
/// coordinates.
pub(crate) fn ground_to_layout(
    point: (f64, f64),
    shot: (u32, u32),
    origin: (i64, i64),
    scale: f64,
) -> (i64, i64) {
    px_to_layout(frame_to_px(point, shot), origin, scale)
}

/// A monitor's layout origin and scale from `hyprctl -j monitors`.
pub(crate) fn monitor_origin(monitors: &Value, name: &str) -> Option<(i64, i64, f64)> {
    let m = monitors
        .as_array()?
        .iter()
        .find(|m| m.get("name").and_then(Value::as_str) == Some(name))?;
    Some((
        m.get("x")?.as_i64()?,
        m.get("y")?.as_i64()?,
        m.get("scale").and_then(Value::as_f64).unwrap_or(1.0),
    ))
}

/// The grounding server: `ANGEL_GROUND_URL` (+ `ANGEL_GROUND_KEY`), the
/// root of an SGLang-style runtime serving the GUI model. A trailing `/v1`
/// is dropped: grounding speaks the native `/generate`, not chat completions.
fn ground_endpoint() -> Result<(String, Option<String>), String> {
    let url = std::env::var("ANGEL_GROUND_URL").map_err(|_| {
        "no grounding model: set ANGEL_GROUND_URL to the runtime serving \
         microsoft/Mage-VL-Cowork (its reference SGLang container)"
            .to_string()
    })?;
    let root = url.trim_end_matches('/');
    let root = root.strip_suffix("/v1").unwrap_or(root).to_string();
    Ok((root, std::env::var("ANGEL_GROUND_KEY").ok()))
}

/// The model's own chat template, rendered for one screenshot turn with
/// thinking off — byte-identical to `chat_template.jinja` for this shape.
/// The reference agent renders its prompt this way and posts it to
/// `/generate`; an OpenAI chat endpoint builds a different prompt, and on
/// the 2026-10-30 live screens that took Bold from 6/12 hits to 0/12.
pub(crate) fn ground_prompt(target: &str) -> String {
    format!(
        "<|im_start|>system\n{GROUND_SYSTEM}<|im_end|>\n<|im_start|>user\n\
         <|vision_start|><|image_pad|><|vision_end|>{}<|im_end|>\n\
         <|im_start|>assistant\n<think>\n\n</think>\n\n",
        ground_user_text(target)
    )
}

/// JPEG q95, the reference agent's screenshot encoding.
fn jpeg_bytes(img: &image::RgbImage) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 95)
        .encode_image(img)
        .map_err(|e| format!("encode jpeg: {e}"))?;
    Ok(buf)
}

/// One look: the model's point on `img`, in `img` pixels.
fn ground_look(img: &image::RgbImage, target: &str) -> Result<(f64, f64), String> {
    let reply = ask_ground_model(&jpeg_bytes(img)?, target)?;
    Ok(frame_to_px(parse_ground_reply(&reply)?, img.dimensions()))
}

/// First look on the whole shot, then the zoom cascade. A zoom pass that
/// gets no usable point keeps the last one; only the first look fails closed.
fn ground_point(shot: &image::RgbImage, target: &str) -> Result<((f64, f64), usize), String> {
    let size = shot.dimensions();
    let mut at = ground_look(shot, target)?;
    let mut passes = 1;
    for zoom in GROUND_ZOOMS {
        let (x0, y0, w, h) = zoom_window(at, size, zoom);
        let crop = image::imageops::crop_imm(shot, x0, y0, w, h).to_image();
        let up = image::imageops::resize(&crop, size.0, size.1, image::imageops::FilterType::Lanczos3);
        let Ok((ux, uy)) = ground_look(&up, target) else {
            break;
        };
        at = (
            f64::from(x0) + ux / f64::from(size.0) * f64::from(w),
            f64::from(y0) + uy / f64::from(size.1) * f64::from(h),
        );
        passes += 1;
    }
    Ok((at, passes))
}

fn ask_ground_model(jpeg: &[u8], target: &str) -> Result<String, String> {
    use base64::Engine as _;
    let (root, key) = ground_endpoint()?;
    let body = json!({
        "text": ground_prompt(target),
        "image_data": [base64::engine::general_purpose::STANDARD.encode(jpeg)],
        "sampling_params": {"temperature": 0.0, "max_new_tokens": 1024}
    });
    let agent = ureq::AgentBuilder::new().timeout(GROUND_TIMEOUT).build();
    let mut req = agent
        .post(&format!("{root}/generate"))
        .set("content-type", "application/json");
    if let Some(key) = key {
        req = req.set("authorization", &format!("Bearer {key}"));
    }
    let reply: Value = req
        .send_json(body)
        .map_err(|e| format!("ground model at {root}: {e}"))?
        .into_json()
        .map_err(|e| format!("ground model reply: {e}"))?;
    reply
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("ground model reply had no text: {reply}"))
}

/// Screenshot file stem: slugified, traversal-free, never empty.
pub(crate) fn shot_stem(name: Option<&str>) -> String {
    let slug: String = name
        .unwrap_or("shot")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    let slug = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "shot".to_string()
    } else {
        slug
    }
}

/// Compact one-line-per-window rendering of `hyprctl -j clients`.
pub(crate) fn format_clients(v: &Value) -> String {
    let Some(clients) = v.as_array() else {
        return "no windows".to_string();
    };
    if clients.is_empty() {
        return "no windows".to_string();
    }
    clients
        .iter()
        .map(|c| {
            let s = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or("");
            let ws = c
                .pointer("/workspace/id")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let at = c.get("at").and_then(Value::as_array);
            let size = c.get("size").and_then(Value::as_array);
            let xy = |a: Option<&Vec<Value>>| {
                a.and_then(|a| Some((a.first()?.as_i64()?, a.get(1)?.as_i64()?)))
                    .map(|(x, y)| format!("{x},{y}"))
                    .unwrap_or_default()
            };
            format!(
                "{} {} {:?} ws{} at {} size {} pid {}",
                s("address"),
                s("class"),
                s("title"),
                ws,
                xy(at),
                xy(size),
                c.get("pid").and_then(Value::as_i64).unwrap_or(0)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join(bin))
            .find(|p| p.is_file())
    })
}

/// Quote a value as a Lua single-quoted string literal for
/// `hyprctl dispatch "hl.dsp.…"` (Hyprland ≥ 0.56 dispatchers are Lua).
pub(crate) fn lua_str(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    format!("'{escaped}'")
}

/// hyprctl reports Lua dispatch failures as `error:`/`warning:` on stdout
/// with exit 0. Treat both as failure so a rejected act never reads as done.
pub(crate) fn check_dispatch(out: &str) -> Result<String, String> {
    let trimmed = out.trim();
    if trimmed.starts_with("error:") || trimmed.contains("warning:") {
        return Err(format!("hyprctl dispatch rejected: {trimmed}"));
    }
    Ok(out.to_string())
}

fn dispatch(lua: &str) -> Result<String, String> {
    check_dispatch(&run_capture("hyprctl", &["dispatch", lua])?)
}

/// The agent's own headless monitor, when one is up: (name, id).
pub(crate) fn agent_display_info(monitors: &Value) -> Option<(String, i64)> {
    monitors.as_array()?.iter().find_map(|m| {
        let name = m.get("name")?.as_str()?;
        if !name.starts_with("HEADLESS") {
            return None;
        }
        Some((name.to_string(), m.get("id")?.as_i64()?))
    })
}

/// Workspace reserved for the agent display.
pub(crate) const AGENT_WORKSPACE: i64 = 90;

/// One correction step toward a click target. ydotool's absolute mode is
/// approximate under pointer acceleration, so clicks move relatively and
/// re-read the true cursor between steps.
pub(crate) fn plan_correction(cur: (i64, i64), target: (i64, i64)) -> (i64, i64, bool) {
    let dx = target.0 - cur.0;
    let dy = target.1 - cur.1;
    (dx, dy, dx.abs() <= 2 && dy.abs() <= 2)
}

fn run_capture(bin: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(bin);
    cmd.args(args);
    let cap = crate::agent::harness::output_timed_fixed_captured(cmd, CMD_TIMEOUT)
        .map_err(|e| format!("{bin}: {e} (is it installed?)"))?;
    if cap.timed_out {
        return Err(format!("{bin} timed out after {}s", CMD_TIMEOUT.as_secs()));
    }
    if !cap.output.status.success() {
        let err = String::from_utf8_lossy(&cap.output.stderr);
        return Err(format!("{bin} {} failed: {}", args.join(" "), err.trim()));
    }
    Ok(String::from_utf8_lossy(&cap.output.stdout).into_owned())
}

fn hands_dir(workspace: &Path) -> PathBuf {
    workspace.join(".angel").join("hands")
}

/// The `present` tool's Stage envelope: the turn loop parses this exact shape
/// and opens the image on the Stage / Scryglass preview. Pure — unit-tested
/// against `presentation_from_result`.
pub(crate) fn present_envelope(label: &str, url: &str, note: &str) -> String {
    json!({
        "status": "queued", "kind": "image", "label": label, "url": url, "note": note
    })
    .to_string()
}

/// Wrap a saved screenshot in the Stage envelope (default) so the operator
/// watches the action land above the cockpit; `present: false` — or a path
/// the workspace-bound presentation check rejects — keeps the plain receipt.
fn presented_shot(workspace: &Path, file: &Path, label: &str, note: String, present: bool) -> String {
    if !present {
        return note;
    }
    match crate::ui::media::model_presentation_target_in(&file.to_string_lossy(), workspace) {
        Ok(url) => present_envelope(label, &url, &note),
        Err(_) => note,
    }
}

fn wants_present(args: &Value) -> bool {
    args.get("present").and_then(Value::as_bool) != Some(false)
}

fn stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// desktop — Hyprland observe + act
// ---------------------------------------------------------------------------

pub(crate) struct DesktopTool {
    workspace: PathBuf,
}

fn monitors_json() -> Result<Value, String> {
    let raw = run_capture("hyprctl", &["-j", "monitors"])?;
    Ok(serde_json::from_str(&raw).unwrap_or(Value::Null))
}

fn cursor_pos() -> Result<(i64, i64), String> {
    let raw = run_capture("hyprctl", &["-j", "cursorpos"])?;
    let v: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    Ok((
        v.get("x").and_then(Value::as_i64).unwrap_or(0),
        v.get("y").and_then(Value::as_i64).unwrap_or(0),
    ))
}

/// Move one window (by address) onto the agent workspace, without following.
fn move_window_to_agent(address: &str) -> Result<String, String> {
    dispatch(&format!(
        "hl.dsp.window.move({{window={}, workspace={}, follow=false}})",
        lua_str(&format!("address:{address}")),
        lua_str(&AGENT_WORKSPACE.to_string())
    ))
}

impl DesktopTool {
    /// Create (or reuse) angel's own headless monitor + workspace 90, bound
    /// by focusing the monitor by id first — the order verified live on
    /// Hyprland 0.56. The operator's real monitors never show it.
    fn agent_start(&self) -> Result<String, String> {
        if let Some((name, id)) = agent_display_info(&monitors_json()?) {
            return Ok(format!(
                "agent display already up: {name} (monitor {id}), workspace {AGENT_WORKSPACE}"
            ));
        }
        check_dispatch(&run_capture("hyprctl", &["output", "create", "headless"])?)?;
        std::thread::sleep(Duration::from_millis(400));
        let (name, id) = agent_display_info(&monitors_json()?)
            .ok_or("headless output created but not visible in monitors")?;
        dispatch(&format!(
            "hl.dsp.focus({{monitor={}}})",
            lua_str(&id.to_string())
        ))?;
        dispatch(&format!(
            "hl.dsp.focus({{workspace={}}})",
            lua_str(&AGENT_WORKSPACE.to_string())
        ))?;
        Ok(format!(
            "agent display up: {name} (monitor {id}), workspace {AGENT_WORKSPACE}. \
             Launch with agent=true, screenshot with agent=true; the operator's \
             screens do not show it. agent_stop removes it."
        ))
    }

    fn agent_stop(&self) -> Result<String, String> {
        match agent_display_info(&monitors_json()?) {
            Some((name, _)) => {
                check_dispatch(&run_capture("hyprctl", &["output", "remove", &name])?)?;
                Ok(format!("agent display removed: {name}"))
            }
            None => Ok("no agent display is up".to_string()),
        }
    }

    fn screenshot(&self, args: &Value) -> Result<String, String> {
        let dir = hands_dir(&self.workspace);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let file = dir.join(format!(
            "{}-{}.png",
            shot_stem(args.get("name").and_then(Value::as_str)),
            stamp()
        ));
        let mut cmd_args: Vec<String> = Vec::new();
        let agent_output = if args.get("agent").and_then(Value::as_bool) == Some(true) {
            Some(
                agent_display_info(&monitors_json()?)
                    .map(|(name, _)| name)
                    .ok_or("no agent display up — run agent_start first")?,
            )
        } else {
            None
        };
        if let Some(output) = agent_output
            .as_deref()
            .or_else(|| args.get("output").and_then(Value::as_str))
        {
            cmd_args.push("-o".into());
            cmd_args.push(output.to_string());
        }
        if let Some(geom) = args.get("geometry").and_then(Value::as_str) {
            cmd_args.push("-g".into());
            cmd_args.push(geom.to_string());
        }
        cmd_args.push(file.to_string_lossy().into_owned());
        let refs: Vec<&str> = cmd_args.iter().map(String::as_str).collect();
        run_capture("grim", &refs)?;
        let bytes = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        let stem = shot_stem(args.get("name").and_then(Value::as_str));
        let note = format!(
            "screenshot saved: {} ({} bytes). Read it with vision_look or /see.",
            file.display(),
            bytes
        );
        Ok(presented_shot(
            &self.workspace,
            &file,
            &format!("desktop · {stem}"),
            note,
            wants_present(args),
        ))
    }

    /// Locate a UI element with the GUI grounding model: screenshot one
    /// output (the agent display with agent=true, else `output`, else the
    /// focused monitor), ask for the click point, map it to layout coords.
    /// Observe-only: the click itself stays a capsule-approved `click`.
    fn ground(&self, args: &Value) -> Result<String, String> {
        let target = args
            .get("target")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or("missing 'target' (e.g. 'the Save button')")?;
        let monitors = monitors_json()?;
        let output = if args.get("agent").and_then(Value::as_bool) == Some(true) {
            agent_display_info(&monitors)
                .map(|(name, _)| name)
                .ok_or("no agent display up — run agent_start first")?
        } else if let Some(o) = args.get("output").and_then(Value::as_str) {
            o.to_string()
        } else {
            monitors
                .as_array()
                .and_then(|ms| {
                    ms.iter()
                        .find(|m| m.get("focused").and_then(Value::as_bool) == Some(true))
                })
                .and_then(|m| m.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .ok_or("no focused monitor to ground on — pass output or agent=true")?
        };
        let (ox, oy, scale) = monitor_origin(&monitors, &output)
            .ok_or_else(|| format!("no monitor named {output:?}"))?;
        let dir = hands_dir(&self.workspace);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let file = dir.join(format!("ground-{}.png", stamp()));
        let path = file.to_string_lossy().into_owned();
        run_capture("grim", &["-o", &output, &path])?;
        let shot = image::open(&file)
            .map_err(|e| format!("read {path}: {e}"))?
            .to_rgb8();
        let (px, passes) = ground_point(&shot, target)?;
        let (x, y) = px_to_layout(px, (ox, oy), scale);
        validate_coords(x, y)?;
        Ok(format!(
            "{target} is at ({x},{y}) on {output} ({passes} looks, zooming in; screenshot \
             {path}). Act with click {{x:{x}, y:{y}}}, then screenshot to verify.",
        ))
    }

    fn click(&self, args: &Value) -> Result<String, String> {
        let x = args.get("x").and_then(Value::as_i64).ok_or("missing 'x'")?;
        let y = args.get("y").and_then(Value::as_i64).ok_or("missing 'y'")?;
        validate_coords(x, y)?;
        let button = args.get("button").and_then(Value::as_str).unwrap_or("left");
        if which("ydotool").is_some() {
            // Relative moves with re-read corrections: ydotool's absolute
            // mode drifts under pointer acceleration (observed live), so
            // converge on the true cursor position instead of trusting it.
            let mut landed = cursor_pos()?;
            for _ in 0..4 {
                let (dx, dy, done) = plan_correction(landed, (x, y));
                if done {
                    break;
                }
                run_capture(
                    "ydotool",
                    &["mousemove", "-x", &dx.to_string(), "-y", &dy.to_string()],
                )?;
                std::thread::sleep(Duration::from_millis(60));
                landed = cursor_pos()?;
            }
            let code = match button {
                "right" => "0xC1",
                "middle" => "0xC2",
                _ => "0xC0",
            };
            run_capture("ydotool", &["click", code])?;
            let off = (landed.0 - x).abs() + (landed.1 - y).abs();
            return Ok(format!(
                "clicked {button} at ({x},{y}); cursor landed at ({},{}){}. \
                 Screenshot to verify the hit.",
                landed.0,
                landed.1,
                if off > 8 {
                    format!(", {off}px off target (pointer acceleration?)")
                } else {
                    String::new()
                }
            ));
        }
        if which("wlrctl").is_some() {
            run_capture(
                "wlrctl",
                &["pointer", "move", &x.to_string(), &y.to_string()],
            )?;
            run_capture("wlrctl", &["pointer", "click", button])?;
            return Ok(format!("clicked {button} at ({x},{y}) via virtual pointer"));
        }
        Err(
            "no mouse backend: install ydotool (`sudo pacman -S ydotool`, then start \
             `ydotoold`) or wlrctl. Keyboard, focus and screenshots already work."
                .to_string(),
        )
    }
}

impl Tool for DesktopTool {
    fn name(&self) -> &str {
        "desktop"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "desktop".to_string(),
            description: "See and operate the operator's Omarchy/Hyprland desktop. Observe: \
                status (monitors, active window, backends), windows, cursor, screenshot {name?, output?=monitor name, geometry?='x,y WxH'} \
                (saved under .angel/hands/ and opened on the Stage preview for the operator; present=false skips the preview; read it yourself with vision_look), \
                ground {target: 'the Save button', agent?, output?} (a GUI model finds it on screen and returns click coords). Act (operator-approved): \
                focus {address|workspace|class}, key {key: 'ctrl+l'}, type {text}, \
                click {x,y,button?}, launch {command: argv array, agent?}. Prefer your own                 agent display: agent_start gives you a headless monitor + workspace 90 the                 operator never sees; launch and screenshot there with agent=true, agent_stop                 removes it. Touch the operator's real screens only when the task is about                 their open apps. Observe first, act second, screenshot to verify."
                .to_string(),
            params: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["status","windows","cursor","screenshot","ground","focus","key","type","click","launch","agent_start","agent_stop"]},
                    "target": {"type": "string"},
                    "name": {"type": "string"},
                    "output": {"type": "string"},
                    "geometry": {"type": "string"},
                    "present": {"type": "boolean"},
                    "agent": {"type": "boolean"},
                    "address": {"type": "string"}, "workspace": {"type": "integer"},
                    "class": {"type": "string"}, "key": {"type": "string"},
                    "text": {"type": "string"}, "x": {"type": "integer"}, "y": {"type": "integer"},
                    "button": {"type": "string", "enum": ["left","right","middle"]},
                    "command": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["action"]
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let action = args
            .get("action")
            .and_then(Value::as_str)
            .ok_or("missing 'action'")?;
        match action {
            "status" => {
                let monitors = run_capture("hyprctl", &["-j", "monitors"])?;
                let active = run_capture("hyprctl", &["-j", "activewindow"]).unwrap_or_default();
                let backends = [
                    "hyprctl", "grim", "wtype", "wl-copy", "ydotool", "wlrctl", "google-chrome-stable", "chromium", "node",
                ]
                .iter()
                .map(|b| format!("{b}={}", if which(b).is_some() { "yes" } else { "MISSING" }))
                .collect::<Vec<_>>()
                .join(" ");
                Ok(format!(
                    "backends: {backends}\nmonitors: {monitors}\nactive: {active}"
                ))
            }
            "windows" => {
                let raw = run_capture("hyprctl", &["-j", "clients"])?;
                let v: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
                Ok(format_clients(&v))
            }
            "cursor" => run_capture("hyprctl", &["-j", "cursorpos"]),
            "screenshot" => self.screenshot(args),
            "ground" => self.ground(args),
            "focus" => {
                // Hyprland 0.56 dispatchers are Lua (`hl.dsp.focus({…})`);
                // the old `focuswindow address:…` form is rejected.
                if let Some(addr) = args.get("address").and_then(Value::as_str) {
                    dispatch(&format!(
                        "hl.dsp.focus({{window={}}})",
                        lua_str(&format!("address:{addr}"))
                    ))?;
                    Ok(format!("focused {addr}"))
                } else if let Some(class) = args.get("class").and_then(Value::as_str) {
                    dispatch(&format!(
                        "hl.dsp.focus({{window={}}})",
                        lua_str(&format!("class:{class}"))
                    ))?;
                    Ok(format!("focused class {class}"))
                } else if let Some(ws) = args.get("workspace").and_then(Value::as_i64) {
                    dispatch(&format!(
                        "hl.dsp.focus({{workspace={}}})",
                        lua_str(&ws.to_string())
                    ))?;
                    Ok(format!("workspace {ws}"))
                } else {
                    Err("focus needs 'address', 'class' or 'workspace'".to_string())
                }
            }
            "agent_start" => self.agent_start(),
            "agent_stop" => self.agent_stop(),
            "key" => {
                let key = args
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or("missing 'key'")?;
                let parts = validate_key(key)?;
                let (mods, last) = parts.split_at(parts.len() - 1);
                let mut a: Vec<String> = Vec::new();
                for m in mods {
                    a.push("-M".into());
                    a.push(m.to_lowercase());
                }
                a.push("-k".into());
                a.push(last[0].clone());
                for m in mods {
                    a.push("-m".into());
                    a.push(m.to_lowercase());
                }
                let refs: Vec<&str> = a.iter().map(String::as_str).collect();
                run_capture("wtype", &refs)?;
                Ok(format!("key {key}"))
            }
            "type" => {
                let text = args
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or("missing 'text'")?;
                validate_type_text(text)?;
                run_capture("wtype", &["--", text])?;
                Ok(format!("typed {} chars", text.chars().count()))
            }
            "click" => self.click(args),
            "launch" => {
                let argv: Vec<String> = args
                    .get("command")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .filter(|a: &Vec<String>| !a.is_empty())
                    .ok_or("launch needs non-empty 'command' argv array")?;
                let child = Command::new(&argv[0])
                    .args(&argv[1..])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| format!("launch {}: {e}", argv[0]))?;
                if args.get("agent").and_then(Value::as_bool) == Some(true) {
                    let pid = child.id() as i64;
                    // Poll briefly for the window, then park it on the
                    // agent workspace without stealing the operator's focus.
                    for _ in 0..10 {
                        std::thread::sleep(Duration::from_millis(300));
                        let raw = run_capture("hyprctl", &["-j", "clients"]).unwrap_or_default();
                        let clients: Value =
                            serde_json::from_str(&raw).unwrap_or(Value::Null);
                        let addr = clients.as_array().and_then(|cs| {
                            cs.iter().find_map(|c| {
                                (c.get("pid").and_then(Value::as_i64) == Some(pid))
                                    .then(|| c.get("address")?.as_str().map(str::to_string))?
                            })
                        });
                        if let Some(addr) = addr {
                            move_window_to_agent(&addr)?;
                            return Ok(format!(
                                "launched {} on the agent display (workspace {AGENT_WORKSPACE})",
                                argv.join(" ")
                            ));
                        }
                    }
                    return Ok(format!(
                        "launched {} but its window never appeared to park on the agent display",
                        argv.join(" ")
                    ));
                }
                Ok(format!("launched {}", argv.join(" ")))
            }
            other => Err(format!("unknown desktop action {other:?}")),
        }
    }
}

// ---------------------------------------------------------------------------
// browser — Chromium over CDP, driven by browser_driver.mjs
// ---------------------------------------------------------------------------

struct BrowserSession {
    chromium: Child,
    _driver: Child,
    stdin: ChildStdin,
    replies: mpsc::Receiver<String>,
    next_id: u64,
    port: u16,
}

pub(crate) struct BrowserTool {
    workspace: PathBuf,
    session: Arc<Mutex<Option<BrowserSession>>>,
}

/// Browser binary preference: the operator's real Chrome first (their
/// signed-in profile lives there), Chromium as fallback. `ANGEL_BROWSER_BIN`
/// overrides with an explicit path or PATH name.
pub(crate) const BROWSER_CANDIDATES: &[&str] = &[
    "google-chrome-stable",
    "google-chrome",
    "chromium",
    "chromium-browser",
];

pub(crate) fn browser_binary() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("ANGEL_BROWSER_BIN") {
        let raw = raw.trim();
        if !raw.is_empty() {
            let path = PathBuf::from(raw);
            if path.is_file() {
                return Some(path);
            }
            if let Some(found) = which(raw) {
                return Some(found);
            }
        }
    }
    BROWSER_CANDIDATES.iter().find_map(|name| which(name))
}

/// Every installed candidate, in preference order, with its `--version`
/// line when the binary answers one. Feeds the `/browser` picker popup.
pub(crate) fn detected_browsers() -> Vec<(String, PathBuf, String)> {
    BROWSER_CANDIDATES
        .iter()
        .filter_map(|name| {
            let path = which(name)?;
            let version = run_capture(&path.to_string_lossy(), &["--version"])
                .map(|v| v.trim().to_string())
                .unwrap_or_default();
            Some((name.to_string(), path, version))
        })
        .collect()
}

/// The operator's saved `/browser` pick for one workspace
/// (`.angel/browser-bin`, one absolute path). `ANGEL_BROWSER_BIN` wins.
pub(crate) fn browser_choice_path(workspace: &Path) -> PathBuf {
    workspace.join(".angel").join("browser-bin")
}

pub(crate) fn read_browser_choice(workspace: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(browser_choice_path(workspace)).ok()?;
    let path = PathBuf::from(raw.trim());
    path.is_file().then_some(path)
}

pub(crate) fn write_browser_choice(
    workspace: &Path,
    choice: Option<&Path>,
) -> Result<(), String> {
    let file = browser_choice_path(workspace);
    match choice {
        Some(path) => {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(&file, path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())
        }
        None => match std::fs::remove_file(&file) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        },
    }
}

/// Resolution order: `ANGEL_BROWSER_BIN`, the saved `/browser` pick for this
/// workspace, then Chrome-first detection.
pub(crate) fn browser_binary_in(workspace: &Path) -> Option<PathBuf> {
    if std::env::var_os("ANGEL_BROWSER_BIN").is_none()
        && let Some(choice) = read_browser_choice(workspace)
    {
        return Some(choice);
    }
    browser_binary()
}

fn free_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    Ok(listener.local_addr().map_err(|e| e.to_string())?.port())
}

impl BrowserTool {
    fn ensure_session(&self, headless: bool) -> Result<(), String> {
        let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let alive = guard
            .as_mut()
            .map(|s| s.chromium.try_wait().ok().flatten().is_none())
            .unwrap_or(false);
        if alive {
            return Ok(());
        }
        let chromium = browser_binary_in(&self.workspace).ok_or(
            "no Chrome/Chromium found (google-chrome-stable, google-chrome, chromium); \
             set ANGEL_BROWSER_BIN to your browser binary",
        )?;
        let node = which("node").ok_or("browser driver needs node >= 22 on PATH")?;
        let port = free_port()?;
        let profile = self.workspace.join(".angel").join("browser-profile");
        std::fs::create_dir_all(&profile).map_err(|e| e.to_string())?;
        let mut cmd = Command::new(&chromium);
        cmd.args([
            &format!("--remote-debugging-port={port}"),
            &format!("--user-data-dir={}", profile.display()),
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-dev-shm-usage",
        ]);
        if headless {
            cmd.arg("--headless=new");
        }
        cmd.arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let chromium_child = cmd
            .spawn()
            .map_err(|e| format!("launch {}: {e}", chromium.display()))?;

        let driver_path = self.workspace.join(".angel").join("browser_driver.mjs");
        std::fs::write(&driver_path, include_str!("browser_driver.mjs"))
            .map_err(|e| format!("write browser driver: {e}"))?;
        let mut driver = Command::new(&node)
            .arg(&driver_path)
            .arg(port.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("launch browser driver: {e}"))?;
        let stdin = driver.stdin.take().ok_or("driver stdin unavailable")?;
        let stdout = driver.stdout.take().ok_or("driver stdout unavailable")?;
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if tx.send(line.clone()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let mut session = BrowserSession {
            chromium: chromium_child,
            _driver: driver,
            stdin,
            replies: rx,
            next_id: 0,
            port,
        };
        // First line is the driver's ready/error report (id 0).
        let ready = session
            .replies
            .recv_timeout(Duration::from_secs(20))
            .map_err(|_| "browser driver never reported ready".to_string())?;
        let v: Value = serde_json::from_str(&ready).unwrap_or(Value::Null);
        if v.get("ok").and_then(Value::as_bool) != Some(true) {
            let _ = session.chromium.kill();
            return Err(format!(
                "browser driver failed: {}",
                v.get("error").and_then(Value::as_str).unwrap_or("unknown")
            ));
        }
        *guard = Some(session);
        Ok(())
    }

    fn request(&self, mut cmd: Value) -> Result<String, String> {
        let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let s = guard
            .as_mut()
            .ok_or("browser not open — use action=open first")?;
        s.next_id += 1;
        let id = s.next_id;
        cmd["id"] = json!(id);
        let line = format!("{cmd}\n");
        s.stdin
            .write_all(line.as_bytes())
            .and_then(|_| s.stdin.flush())
            .map_err(|e| format!("browser driver write: {e}"))?;
        let deadline = Instant::now() + DRIVER_TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err("browser driver timed out".to_string());
            }
            let reply = s
                .replies
                .recv_timeout(left)
                .map_err(|_| "browser driver timed out".to_string())?;
            let v: Value = serde_json::from_str(&reply).unwrap_or(Value::Null);
            if v.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            return if v.get("ok").and_then(Value::as_bool) == Some(true) {
                Ok(v.get("result")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string())
            } else {
                Err(v
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("browser error")
                    .to_string())
            };
        }
    }

    /// Park the session's Chrome window on the agent workspace (best
    /// effort: no agent display, no move). Finds the window by the
    /// spawned browser's pid in `hyprctl -j clients`.
    fn park_on_agent_display(&self) -> Result<(), String> {
        if agent_display_info(&monitors_json()?).is_none() {
            return Ok(());
        }
        let pid = {
            let guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
            guard.as_ref().map(|s| s.chromium.id() as i64)
        }
        .ok_or("browser not open")?;
        let raw = run_capture("hyprctl", &["-j", "clients"])?;
        let clients: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
        if let Some(addr) = clients.as_array().and_then(|cs| {
            cs.iter().find_map(|c| {
                (c.get("pid").and_then(Value::as_i64) == Some(pid))
                    .then(|| c.get("address")?.as_str().map(str::to_string))?
            })
        }) {
            move_window_to_agent(&addr)?;
        }
        Ok(())
    }

    fn close(&self) -> Result<String, String> {
        let mut guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        match guard.take() {
            Some(mut s) => {
                let _ = s.chromium.kill();
                Ok(format!("browser closed (was on CDP port {})", s.port))
            }
            None => Ok("browser was not open".to_string()),
        }
    }
}

impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "browser".to_string(),
            description: "Drive the operator's Chrome (or Chromium fallback) over CDP with a persistent session. Act \
                (operator-approved): open {url, headed?} starts/reuses the browser and \
                navigates; click {selector|text}; type {selector, text}; press {key: 'Enter'}; \
                eval {script}; close. Observe: status, snapshot (title, url, interactive \
                elements with selectors, page text), screenshot (saved under .angel/hands/ and opened on the Stage preview for the operator; present=false skips the preview). \
                Only http(s)/about:blank. Snapshot after every act to verify."
                .to_string(),
            params: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["open","status","snapshot","click","type","press","eval","screenshot","close"]},
                    "url": {"type": "string"}, "headed": {"type": "boolean"},
                    "agent": {"type": "boolean"},
                    "selector": {"type": "string"}, "text": {"type": "string"},
                    "key": {"type": "string"}, "script": {"type": "string"},
                    "name": {"type": "string"},
                    "present": {"type": "boolean"},
                    "wait_ms": {"type": "integer"}
                },
                "required": ["action"]
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let action = args
            .get("action")
            .and_then(Value::as_str)
            .ok_or("missing 'action'")?;
        match action {
            "status" => {
                let guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
                Ok(match guard.as_ref() {
                    Some(s) => format!(
                        "browser open on CDP port {} (profile .angel/browser-profile)",
                        s.port
                    ),
                    None => format!(
                        "browser closed. browser={} node={}",
                        browser_binary_in(&self.workspace)
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "MISSING".into()),
                        which("node")
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "MISSING".into())
                    ),
                })
            }
            "open" => {
                let url = validate_browser_url(
                    args.get("url")
                        .and_then(Value::as_str)
                        .unwrap_or("about:blank"),
                )?;
                let headed = args
                    .get("headed")
                    .and_then(Value::as_bool)
                    .unwrap_or_else(|| {
                        std::env::var_os("WAYLAND_DISPLAY").is_some()
                            && !env_flag("ANGEL_BROWSER_HEADLESS", false)
                    });
                self.ensure_session(!headed)?;
                // Default to the agent display when one is up: the headed
                // window lives on workspace 90, off the operator's screens.
                // Explicit agent=false keeps it in front of the operator.
                let agent = args.get("agent").and_then(Value::as_bool);
                if headed && agent != Some(false) {
                    let _ = self.park_on_agent_display();
                }
                self.request(json!({"action": "goto", "url": url, "wait_ms": args.get("wait_ms")}))
            }
            "snapshot" => self.request(json!({"action": "snapshot"})),
            "click" => self.request(json!({
                "action": "click",
                "selector": args.get("selector"), "text": args.get("text"),
                "wait_ms": args.get("wait_ms")
            })),
            "type" => {
                let text = args
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or("missing 'text'")?;
                validate_type_text(text)?;
                let selector = args
                    .get("selector")
                    .and_then(Value::as_str)
                    .ok_or("missing 'selector'")?;
                self.request(json!({"action": "type", "selector": selector, "text": text}))
            }
            "press" => {
                let key = args
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or("missing 'key'")?;
                self.request(json!({"action": "press", "key": key, "wait_ms": args.get("wait_ms")}))
            }
            "eval" => {
                let script = args
                    .get("script")
                    .and_then(Value::as_str)
                    .ok_or("missing 'script'")?;
                self.request(json!({"action": "eval", "script": script}))
            }
            "screenshot" => {
                let dir = hands_dir(&self.workspace);
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let stem = shot_stem(args.get("name").and_then(Value::as_str));
                let file = dir.join(format!("{}-{}.png", stem, stamp()));
                self.request(json!({"action": "screenshot", "path": file.to_string_lossy()}))?;
                let bytes = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
                let note = format!(
                    "browser screenshot saved: {} ({} bytes). Read it with vision_look or /see.",
                    file.display(),
                    bytes
                );
                Ok(presented_shot(
                    &self.workspace,
                    &file,
                    &format!("browser · {stem}"),
                    note,
                    wants_present(args),
                ))
            }
            "close" => self.close(),
            other => Err(format!("unknown browser action {other:?}")),
        }
    }
}

/// Register `desktop` + `browser`, gated on `ANGEL_HANDS` (default on) and a
/// Wayland session. Headless/task contexts without a display simply don't
/// advertise hands they cannot use.
pub(crate) fn maybe_register_hands(r: &mut ToolRegistry, workspace: PathBuf) {
    if !env_flag("ANGEL_HANDS", true) {
        return;
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_none()
        && std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none()
    {
        return;
    }
    r.register(Box::new(DesktopTool {
        workspace: workspace.clone(),
    }));
    r.register(Box::new(BrowserTool {
        workspace,
        session: Arc::new(Mutex::new(None)),
    }));
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/hands__tests.rs"]
mod tests;
