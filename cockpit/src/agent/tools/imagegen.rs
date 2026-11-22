//! Calls the installed Codex CLI's ImageGen with its ChatGPT login.
use crate::agent::club::ToolDef;
use crate::agent::harness::{
    Tool, ToolRegistry, confined_publish_new_no_symlinks, confined_read_dir,
    confined_read_limited_no_symlinks, workspace_relative,
};
use crate::agent::openai_codex::{ChatGptAuth, codex_home_path};
use crate::agent::service_process::ServiceChild;
use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const PNG_HEADER: &[u8] = b"\x89PNG\r\n\x1a\n";

pub(crate) fn maybe_register(registry: &mut ToolRegistry, workspace: &Path) {
    if crate::agent::sandbox::sealed::identity().is_none()
        && ChatGptAuth::has_local_login()
        && let Some(executable) = codex_executable()
    {
        registry.register(Box::new(ImageGen {
            executable,
            workspace: workspace.to_path_buf(),
            generated_root: codex_home_path().join("generated_images"),
        }));
    }
}

fn codex_executable() -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let search_path = std::env::var_os("PATH").unwrap_or_default();
    let user_bin = std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/bin"));
    std::env::split_paths(&search_path)
        .chain(user_bin)
        .map(|dir| dir.join("codex"))
        .find(|candidate| {
            candidate
                .metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
        .and_then(|candidate| std::path::absolute(candidate).ok())
}

struct ImageGen {
    executable: PathBuf,
    workspace: PathBuf,
    generated_root: PathBuf,
}

impl ImageGen {
    fn local_target(&self, path: &str) -> Result<(String, PathBuf), String> {
        let target = crate::ui::media::model_presentation_target_in(path, &self.workspace)?;
        let absolute = Path::new(&target);
        if !absolute.is_absolute() {
            return Err("imagegen requires a local workspace path".into());
        }
        let relative = workspace_relative(&self.workspace, absolute)?;
        Ok((target, relative))
    }
}

impl Tool for ImageGen {
    fn name(&self) -> &str {
        "imagegen"
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name().into(),
            description: "Generate or edit a raster image with Codex ImageGen using the signed-in ChatGPT account. Use for user requests to draw, create, or edit images, including artwork for slides. Saves a PNG and shows it on the Stage. Pass workspace image paths for edits or references.".into(),
            params: json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Describe the desired image, or the changes to make to the reference images."},
                    "path": {"type": "string", "description": "New workspace-relative PNG path, such as artwork/cover.png."},
                    "label": {"type": "string", "description": "Optional short caption for the Stage."},
                    "references": {"type": "array", "maxItems": 5, "items": {"type": "string"}, "description": "Optional workspace paths to reference images."},
                    "transparent_background": {"type": "boolean", "description": "Request transparency when true."}
                },
                "required": ["prompt", "path"],
                "additionalProperties": false
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&AtomicBool>,
    ) -> Result<String, String> {
        check_cancel(cancel)?;
        let prompt = args["prompt"]
            .as_str()
            .map(str::trim)
            .filter(|prompt| !prompt.is_empty() && prompt.len() <= 16_384)
            .ok_or("imagegen requires a prompt of 1–16,384 bytes")?;
        let path = args["path"]
            .as_str()
            .ok_or("imagegen requires a PNG output path")?;
        let (target, relative) = self.local_target(path)?;
        if !relative
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        {
            return Err("imagegen output path must end in .png".into());
        }
        if fs::symlink_metadata(&target).is_ok() {
            return Err("imagegen output already exists. Choose a new PNG path.".into());
        }
        let label = args["label"].as_str().unwrap_or("Generated image");
        if !crate::ui::media::presentation_text_valid(label, 512) {
            return Err(
                "imagegen label must contain 1–512 bytes without control characters".into(),
            );
        }

        let scratch = Scratch::new()?;
        let references = match args.get("references") {
            None => &[][..],
            Some(Value::Array(references)) if references.len() <= 5 => references.as_slice(),
            _ => return Err("imagegen accepts up to five reference image paths".into()),
        };
        let mut command = Command::new(&self.executable);
        command
            .current_dir(&scratch.0)
            .args([
                "exec",
                "--json",
                "--ephemeral",
                "--ignore-user-config",
                "--ignore-rules",
                "--skip-git-repo-check",
                "--sandbox",
                "read-only",
                "--enable",
                "image_generation",
                "--disable",
                "shell_tool",
                "--disable",
                "view_image",
                "--disable",
                "apps",
                "--disable",
                "plugins",
                "--disable",
                "hooks",
                "--disable",
                "skill_search",
                "-c",
                "approval_policy=\"never\"",
                "-c",
                "forced_login_method=\"chatgpt\"",
                "-c",
                "web_search=\"disabled\"",
            ])
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY");
        for (index, reference) in references.iter().enumerate() {
            let path = reference
                .as_str()
                .ok_or("invalid imagegen reference path")?;
            let (_, relative) = self.local_target(path)?;
            let bytes =
                confined_read_limited_no_symlinks(&self.workspace, &relative, MAX_IMAGE_BYTES)?
                    .ok_or("imagegen reference exceeds 32 MiB")?;
            let format = image::guess_format(&bytes)
                .map_err(|_| "imagegen reference is not a recognized image")?;
            let staged = scratch
                .0
                .join(format!("reference-{index}.{}", format.extensions_str()[0]));
            fs::write(&staged, bytes)
                .map_err(|error| format!("stage imagegen reference: {error}"))?;
            command.arg("--image").arg(staged);
        }

        let background = if args["transparent_background"].as_bool() == Some(true) {
            "Set transparent_background to true."
        } else {
            "Use an opaque background."
        };
        let instruction = format!(
            "Call the built-in Codex image generation tool exactly once and generate a PNG. {background} \
             If images are attached, pass them with num_last_images_to_include. Use only the image tool. \
             The image itself is the deliverable.\n\nImage request:\n{prompt}"
        );
        let stdin_path = scratch.0.join("request.txt");
        fs::write(&stdin_path, instruction)
            .map_err(|error| format!("stage imagegen prompt: {error}"))?;
        let stdin =
            File::open(stdin_path).map_err(|error| format!("open imagegen prompt: {error}"))?;
        command
            .args(["--", "-"])
            .stdin(stdin)
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        // Codex exec reports the thread ID, while ImageGen writes the PNG in
        // that thread's generated_images directory.
        let thread_id = run_codex(command, cancel)?;
        let bytes = read_generated_image(&self.generated_root, &thread_id)?;
        check_cancel(cancel)?;
        confined_publish_new_no_symlinks(&self.workspace, &relative, &bytes)?;
        Ok(json!({"status":"queued", "kind":"image", "label":label, "url":target}).to_string())
    }
}

fn check_cancel(cancel: Option<&AtomicBool>) -> Result<(), String> {
    if cancel.is_some_and(|cancel| cancel.load(Ordering::Acquire)) {
        return Err("imagegen cancelled".into());
    }
    Ok(())
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self, String> {
        use ring::rand::{SecureRandom, SystemRandom};
        use std::os::unix::fs::DirBuilderExt;
        let mut nonce = [0u8; 16];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| "could not name imagegen scratch directory")?;
        let nonce = u128::from_ne_bytes(nonce);
        let path = std::env::temp_dir().join(format!("angel-imagegen-{nonce:032x}"));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|error| format!("create imagegen scratch directory: {error}"))?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_codex(mut command: Command, cancel: Option<&AtomicBool>) -> Result<String, String> {
    check_cancel(cancel)?;
    let mut child = ServiceChild::spawn(&mut command)
        .map_err(|error| format!("start Codex ImageGen: {error}"))?;
    let stdout = child
        .take_stdout()
        .ok_or("Codex ImageGen has no output stream")?;
    let (sender, events) = mpsc::sync_channel(8);
    std::thread::Builder::new()
        .name("codex-imagegen-output".into())
        .spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                if !matches!(
                    (&mut reader).take(1024 * 1024).read_line(&mut line),
                    Ok(1..)
                ) || !line.ends_with('\n')
                    || sender.send(line).is_err()
                {
                    break;
                }
            }
        })
        .map_err(|error| format!("read Codex ImageGen output: {error}"))?;

    let deadline = Instant::now()
        + crate::agent::harness::exec::tool_timeout().unwrap_or(Duration::from_secs(300));
    let mut thread_id = None;
    loop {
        check_cancel(cancel)?;
        if Instant::now() >= deadline {
            return Err("Codex ImageGen timed out".into());
        }
        let line = match events.recv_timeout(Duration::from_millis(50)) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Codex ImageGen exited early. Check that your Codex CLI is up to date and signed in with ChatGPT.".into());
            }
        };
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match event["type"].as_str() {
            Some("thread.started") => thread_id = event["thread_id"].as_str().map(str::to_owned),
            Some("turn.completed") => {
                return thread_id.ok_or("Codex returned no image thread ID".into());
            }
            Some("turn.failed") => {
                let detail = event
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("Codex could not complete image generation");
                return Err(detail.chars().take(512).collect());
            }
            _ => {}
        }
    }
}

fn read_generated_image(root: &Path, thread_id: &str) -> Result<Vec<u8>, String> {
    if thread_id.len() != 36
        || !thread_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return Err("Codex returned an invalid image thread ID".into());
    }
    let thread = Path::new(thread_id);
    let root = root
        .canonicalize()
        .map_err(|_| "Codex did not save a generated image")?;
    let entries =
        confined_read_dir(&root, thread).map_err(|_| "Codex did not save a generated image")?;
    let mut pngs = entries.iter().filter(|entry| {
        !entry.is_dir
            && Path::new(&entry.name)
                .extension()
                .is_some_and(|extension| extension == "png")
    });
    let png = pngs.next().ok_or("Codex did not save a generated PNG")?;
    if pngs.next().is_some() {
        return Err("Codex returned multiple images for a single image request".into());
    }
    let bytes = confined_read_limited_no_symlinks(&root, &thread.join(&png.name), MAX_IMAGE_BYTES)?
        .filter(|bytes| bytes.starts_with(PNG_HEADER))
        .ok_or("Codex returned an invalid or oversized PNG")?;
    image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Png)
        .into_dimensions()
        .map_err(|_| "Codex returned an invalid PNG")?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/imagegen__tests.rs"]
mod tests;
