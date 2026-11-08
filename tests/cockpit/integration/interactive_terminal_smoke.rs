#![cfg(unix)]

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::fs;
use std::io::{Read, Write};
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const STEP_TIMEOUT: Duration = Duration::from_secs(4);
const EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CAPTURE_BYTES: usize = 4 * 1024 * 1024;
const DSR_QUERY: &[u8] = b"\x1b[6n";
const DSR_RESPONSE: &[u8] = b"\x1b[1;1R";

struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(label: &str) -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("angelX-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).map_err(|error| format!("create {}: {error}", path.display()))?;
        for child in ["home", "work", "sessions", "config", "cache", "state"] {
            fs::create_dir(path.join(child))
                .map_err(|error| format!("create scratch {child}: {error}"))?;
        }
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn provision_codex_route_fixture(&self) -> Result<(), String> {
        let codex_home = self.0.join("home/.codex");
        fs::create_dir_all(&codex_home)
            .map_err(|error| format!("create {}: {error}", codex_home.display()))?;
        fs::write(
            codex_home.join("auth.json"),
            r#"{
  "auth_mode": "chatgpt",
  "tokens": {
    "access_token": "pty-fixture-access",
    "refresh_token": "pty-fixture-refresh",
    "account_id": "pty-fixture-account"
  }
}
"#,
        )
        .map_err(|error| format!("write PTY Codex auth fixture: {error}"))?;
        fs::write(
            codex_home.join("config.toml"),
            "model = \"pty-sol\"\nmodel_reasoning_effort = \"low\"\n",
        )
        .map_err(|error| format!("write PTY Codex config fixture: {error}"))?;
        fs::write(
            codex_home.join("models_cache.json"),
            r#"{
  "models": [{
    "slug": "pty-sol",
    "display_name": "PTY Sol",
    "visibility": "list",
    "priority": 1,
    "context_window": 128000,
    "default_reasoning_level": "low",
    "supported_reasoning_levels": [
      {"effort": "low", "description": "fixture low"},
      {"effort": "high", "description": "fixture high"}
    ]
  }]
}
"#,
        )
        .map_err(|error| format!("write PTY Codex model catalog fixture: {error}"))?;
        Ok(())
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct CockpitPty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    chunks: Receiver<Vec<u8>>,
    parser: vt100::Parser,
    raw: Vec<u8>,
    dsr_tail: Vec<u8>,
    overflowed: bool,
}

impl CockpitPty {
    fn spawn(rows: u16, cols: u16, scratch: &ScratchDir) -> Result<Self, String> {
        Self::spawn_with_args(rows, cols, scratch, &[])
    }

    fn spawn_with_args(
        rows: u16,
        cols: u16,
        scratch: &ScratchDir,
        args: &[&str],
    ) -> Result<Self, String> {
        Self::spawn_with_motion(rows, cols, scratch, args, "off")
    }

    fn spawn_with_motion(
        rows: u16,
        cols: u16,
        scratch: &ScratchDir,
        args: &[&str],
        motion: &str,
    ) -> Result<Self, String> {
        Self::spawn_with_driver(rows, cols, scratch, args, motion, "practice")
    }

    fn spawn_with_driver(
        rows: u16,
        cols: u16,
        scratch: &ScratchDir,
        args: &[&str],
        motion: &str,
        driver: &str,
    ) -> Result<Self, String> {
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_angel"));
        command.args(args);
        command.env_clear();
        command.cwd(scratch.path().join("work"));
        command.env("TERM", "xterm-256color");
        command.env("LANG", "C.UTF-8");
        command.env("LC_ALL", "C.UTF-8");
        command.env(
            "PATH",
            std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()),
        );
        command.env("HOME", scratch.path().join("home"));
        command.env("XDG_CONFIG_HOME", scratch.path().join("config"));
        command.env("XDG_CACHE_HOME", scratch.path().join("cache"));
        if let Some(compiler) = std::env::var_os("ANGEL_BEND_COMPILER") {
            command.env("ANGEL_BEND_COMPILER", compiler);
        }
        command.env("XDG_STATE_HOME", scratch.path().join("state"));
        command.env("ANGEL_WORKSPACE", scratch.path().join("work"));
        command.env("ANGEL_SESSION_DIR", scratch.path().join("sessions"));
        command.env("ANGEL_MEMORY_FILE", scratch.path().join("memory.json"));
        command.env("ANGEL_GOAL_FILE", scratch.path().join("goal.json"));
        command.env("ANGEL_LOOP_FILE", scratch.path().join("loop.json"));
        command.env(
            "ANGEL_WORK_CONTEXT_DIR",
            scratch.path().join("work-context"),
        );
        command.env("ANGEL_DRIVER", driver);
        command.env("ANGEL_SCAN", "0");
        command.env("ANGEL_SCAN_INTERVAL_SECS", "0");
        command.env("ANGEL_BAG_PROBE", "0");
        command.env("ANGEL_HYDRA_DISCOVER", "0");
        command.env("ANGEL_HYDRA_PUBLISH", "0");
        command.env("ANGEL_LSP", "0");
        command.env("ANGEL_EXPERIENCE", "0");
        command.env("ANGEL_DOSSIER", "0");
        command.env("ANGEL_ROUTE_MEMORY", "0");
        command.env("ANGEL_TUI_MOTION", motion);
        command.env("ANGEL_IMAGE_PROTOCOL", "halfblocks");
        command.env("ANGEL_OVERWATCH_CMD", "true");
        if scratch.path().join("delve-engine").is_file() {
            command.env("ANGEL_DELVE_SYSTEM", scratch.path().join("delve-engine"));
        } else {
            command.env("ANGEL_HOT_SYSTEMS", "0");
        }
        if scratch.path().join("live").is_dir() {
            command.env("ANGEL_LIVE_DIR", scratch.path().join("live"));
        }

        Self::spawn_command(rows, cols, command)
    }

    fn spawn_omp(
        rows: u16,
        cols: u16,
        scratch: &ScratchDir,
        binary: &Path,
    ) -> Result<Self, String> {
        let agent_dir = scratch.path().join("home/.omp/agent");
        fs::create_dir_all(&agent_dir)
            .map_err(|error| format!("create OMP agent dir {}: {error}", agent_dir.display()))?;
        fs::write(
            agent_dir.join("config.yml"),
            "setupVersion: 1\nstartup:\n  setupWizard: false\n  checkUpdate: false\n",
        )
        .map_err(|error| format!("write OMP benchmark config: {error}"))?;

        let mut command = CommandBuilder::new(binary);
        command.args(["--no-session", "--no-title"]);
        command.env_clear();
        command.cwd(scratch.path().join("work"));
        command.env("TERM", "xterm-256color");
        command.env("LANG", "C.UTF-8");
        command.env("LC_ALL", "C.UTF-8");
        command.env(
            "PATH",
            std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()),
        );
        command.env("HOME", scratch.path().join("home"));
        command.env("XDG_CONFIG_HOME", scratch.path().join("config"));
        command.env("XDG_CACHE_HOME", scratch.path().join("cache"));
        command.env("XDG_STATE_HOME", scratch.path().join("state"));
        command.env("PI_CODING_AGENT_DIR", &agent_dir);

        Self::spawn_command(rows, cols, command)
    }

    fn spawn_command(rows: u16, cols: u16, command: CommandBuilder) -> Result<Self, String> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("open PTY: {error}"))?;

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("spawn cockpit: {error}"))?;
        drop(pair.slave);
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("clone PTY reader: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("take PTY writer: {error}"))?;
        let (tx, chunks) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buffer = [0_u8; 16 * 1024];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        if tx.send(buffer[..count].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self {
            master: pair.master,
            writer,
            child,
            chunks,
            parser: vt100::Parser::new(rows, cols, 0),
            raw: Vec::new(),
            dsr_tail: Vec::new(),
            overflowed: false,
        })
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.writer
            .write_all(bytes)
            .and_then(|_| self.writer.flush())
            .map_err(|error| format!("write PTY input: {error}"))
    }

    /// Measure from a flushed PTY write until the expected cells are present in
    /// the terminal parser. This includes the real event loop, frame build,
    /// ratatui diff, terminal write, PTY transport, and VT application rather
    /// than timing only the in-memory composer mutation.
    fn send_until_visible(
        &mut self,
        bytes: &[u8],
        label: &str,
        predicate: impl Fn(&str) -> bool,
    ) -> Result<Duration, String> {
        let started = Instant::now();
        self.send(bytes)?;
        self.wait_for(label, predicate)?;
        Ok(started.elapsed())
    }

    // Start/resume can build a room before the event loop accepts game keys.
    // Observe the expanded viewport before sending a cast or returning to code.
    fn wait_for_game_view(&mut self) -> Result<(), String> {
        self.wait_for("expanded game viewport", |screen| {
            !screen.trim().is_empty() && !screen.contains("Write a message")
        })?;
        Ok(())
    }

    fn drain_for(&mut self, duration: Duration) -> Result<(), String> {
        let deadline = Instant::now() + duration;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match self.chunks.recv_timeout(left) {
                Ok(chunk) => self.process(&chunk)?,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("PTY closed during work probe".into());
                }
            }
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn cpu_ticks(&self) -> Result<u64, String> {
        let pid = self.child.process_id().ok_or("child PID unavailable")?;
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).map_err(|e| e.to_string())?;
        let fields: Vec<_> = stat
            .rsplit_once(')')
            .ok_or("invalid process stat")?
            .1
            .split_whitespace()
            .collect();
        Ok(fields[11].parse::<u64>().map_err(|e| e.to_string())?
            + fields[12].parse::<u64>().map_err(|e| e.to_string())?)
    }

    fn resize(&mut self, rows: u16, cols: u16) -> Result<(), String> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("resize PTY: {error}"))?;
        self.parser.screen_mut().set_size(rows, cols);
        Ok(())
    }

    fn terminal_local_flags(&self) -> Result<libc::tcflag_t, String> {
        let fd = self
            .master
            .as_raw_fd()
            .ok_or("PTY master does not expose a Unix file descriptor")?;
        let mut termios = MaybeUninit::<libc::termios>::uninit();
        // SAFETY: tcgetattr initializes the pointed-to termios value on success,
        // and `fd` remains owned by `self.master` for this call.
        if unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) } != 0 {
            return Err(format!(
                "read PTY terminal mode: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: the successful tcgetattr call above initialized the value.
        Ok(unsafe { termios.assume_init() }.c_lflag)
    }

    fn raw_mode_active(&self) -> Result<bool, String> {
        let flags = self.terminal_local_flags()?;
        Ok(flags & (libc::ICANON | libc::ECHO) == 0)
    }

    fn cooked_mode_restored(&self) -> Result<bool, String> {
        let flags = self.terminal_local_flags()?;
        let cooked = libc::ICANON | libc::ECHO;
        Ok(flags & cooked == cooked)
    }

    fn process(&mut self, chunk: &[u8]) -> Result<(), String> {
        if self.raw.len().saturating_add(chunk.len()) > MAX_CAPTURE_BYTES {
            self.overflowed = true;
        } else {
            self.raw.extend_from_slice(chunk);
        }
        self.parser.process(chunk);
        self.dsr_tail.extend_from_slice(chunk);
        while let Some(offset) = find_bytes(&self.dsr_tail, DSR_QUERY) {
            self.send(DSR_RESPONSE)?;
            self.dsr_tail.drain(..offset + DSR_QUERY.len());
        }
        let keep = DSR_QUERY.len().saturating_sub(1);
        if self.dsr_tail.len() > keep {
            self.dsr_tail.drain(..self.dsr_tail.len() - keep);
        }
        Ok(())
    }

    fn wait_for(
        &mut self,
        label: &str,
        predicate: impl Fn(&str) -> bool,
    ) -> Result<String, String> {
        let deadline = Instant::now() + STEP_TIMEOUT;
        loop {
            let screen = self.parser.screen().contents();
            if predicate(&screen) {
                return Ok(screen);
            }
            if self.overflowed {
                return Err(format!(
                    "{label}: terminal capture exceeded {MAX_CAPTURE_BYTES} bytes"
                ));
            }
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|error| format!("poll cockpit: {error}"))?
            {
                return Err(format!(
                    "{label}: cockpit exited early ({status:?})\n{}",
                    self.diagnostics()
                ));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!("{label}: timed out\n{}", self.diagnostics()));
            }
            match self
                .chunks
                .recv_timeout(remaining.min(Duration::from_millis(100)))
            {
                Ok(chunk) => self.process(&chunk)?,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(format!(
                        "{label}: PTY output closed\n{}",
                        self.diagnostics()
                    ));
                }
            }
        }
    }

    fn finish(self) -> Result<Vec<u8>, String> {
        self.finish_with(b"\x15exit\r", "typed exit")
    }

    fn finish_with(mut self, input: &[u8], action: &str) -> Result<Vec<u8>, String> {
        self.send(input)?;
        let deadline = Instant::now() + EXIT_TIMEOUT;
        let status = loop {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|error| format!("poll cockpit exit: {error}"))?
            {
                break status;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                let diagnostics = self.diagnostics();
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err(format!(
                    "process did not exit after {action}\n{diagnostics}"
                ));
            }
            match self
                .chunks
                .recv_timeout(remaining.min(Duration::from_millis(100)))
            {
                Ok(chunk) => self.process(&chunk)?,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    if let Some(status) = self
                        .child
                        .try_wait()
                        .map_err(|error| format!("poll closed cockpit: {error}"))?
                    {
                        break status;
                    }
                }
            }
        };
        while let Ok(chunk) = self.chunks.try_recv() {
            self.process(&chunk)?;
        }
        if !status.success() {
            return Err(format!(
                "cockpit exit status was {status:?}\n{}",
                self.diagnostics()
            ));
        }
        if !self.cooked_mode_restored()? {
            return Err(
                "cockpit exited without restoring canonical/echo terminal mode".to_string(),
            );
        }
        Ok(std::mem::take(&mut self.raw))
    }

    fn diagnostics(&self) -> String {
        let screen = self.parser.screen().contents();
        let raw_tail = &self.raw[self.raw.len().saturating_sub(2_000)..];
        format!(
            "screen:\n{screen}\nraw tail:\n{}",
            String::from_utf8_lossy(raw_tail)
        )
    }
}

impl Drop for CockpitPty {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn assert_terminal_restored(raw: &[u8]) -> Result<(), String> {
    let entered = find_bytes(raw, b"\x1b[?1049h").ok_or("alternate screen was never entered")?;
    let paste_disabled = find_bytes(raw, b"\x1b[?2004l")
        .ok_or("bracketed paste was not disabled during teardown")?;
    let mouse_disabled =
        find_bytes(raw, b"\x1b[?1000l").ok_or("mouse capture was not disabled during teardown")?;
    let left = find_bytes(raw, b"\x1b[?1049l").ok_or("alternate screen was never left")?;
    if !(entered < paste_disabled
        && entered < mouse_disabled
        && paste_disabled < left
        && mouse_disabled < left)
    {
        return Err("terminal restore sequences were emitted out of lifecycle order".to_string());
    }
    let text = String::from_utf8_lossy(raw);
    if !text.contains("RUN: entering event loop") || !text.contains("RUN: loop exited cleanly") {
        return Err("ordinary cockpit event-loop lifecycle markers were incomplete".to_string());
    }
    if text.contains("panicked at") || text.contains("Error: Custom") {
        return Err("cockpit emitted a panic/error during PTY smoke".to_string());
    }
    Ok(())
}

fn strict_pty_smoke() -> bool {
    std::env::var("ANGEL_PTY_SMOKE_STRICT").is_ok_and(|value| value == "1")
        || std::env::var_os("CI").is_some()
}

fn wait_for_saved_turn(
    scratch: &ScratchDir,
    user_text: &str,
    assistant_text: &str,
) -> Result<PathBuf, String> {
    let session_dir = scratch.path().join("sessions");
    let deadline = Instant::now() + STEP_TIMEOUT;
    loop {
        let entries = fs::read_dir(&session_dir)
            .map_err(|error| format!("read {}: {error}", session_dir.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(record) = serde_json::from_str::<serde_json::Value>(&raw) else {
                continue;
            };
            let Some(history) = record.get("history").and_then(|value| value.as_array()) else {
                continue;
            };
            let has_user = history.iter().any(|message| {
                message.get("role").and_then(|value| value.as_str()) == Some("User")
                    && message.get("content").and_then(|value| value.as_str()) == Some(user_text)
            });
            let has_assistant = history.iter().any(|message| {
                message.get("role").and_then(|value| value.as_str()) == Some("Assistant")
                    && message.get("content").and_then(|value| value.as_str())
                        == Some(assistant_text)
            });
            let workspace_matches = record
                .get("workspace")
                .and_then(|value| value.as_str())
                .is_some_and(|workspace| Path::new(workspace) == scratch.path().join("work"));
            if has_user && has_assistant && workspace_matches {
                return Ok(path);
            }
        }
        if Instant::now() >= deadline {
            let files = fs::read_dir(&session_dir)
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|entry| entry.file_name().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            return Err(format!(
                "session snapshot did not contain the completed bound turn; files={files:?}"
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn write_visual_fixture(scratch: &ScratchDir) -> Result<PathBuf, String> {
    let path = scratch.path().join("work").join("pty-visual-proof.png");
    let image = image::RgbaImage::from_fn(96, 64, |x, y| {
        let checker = ((x / 8) + (y / 8)) % 2 == 0;
        let diagonal = x.abs_diff(y.saturating_mul(3) / 2) < 4;
        if diagonal {
            image::Rgba([255, 220, 96, 255])
        } else if checker {
            image::Rgba([24, 220, 224, 255])
        } else {
            image::Rgba([12, 18, 24, 255])
        }
    });
    image
        .save(&path)
        .map_err(|error| format!("write {}: {error}", path.display()))?;
    Ok(path)
}

fn fixture_color_cells(screen: &vt100::Screen, color: vt100::Color) -> usize {
    let (rows, columns) = screen.size();
    (0..rows)
        .flat_map(|row| (0..columns).filter_map(move |column| screen.cell(row, column)))
        .filter(|cell| cell.fgcolor() == color || cell.bgcolor() == color)
        .count()
}

fn visual_fixture_color_counts(screen: &vt100::Screen) -> (usize, usize, usize) {
    (
        fixture_color_cells(screen, vt100::Color::Rgb(24, 220, 224)),
        fixture_color_cells(screen, vt100::Color::Rgb(255, 220, 96)),
        fixture_color_cells(screen, vt100::Color::Rgb(12, 18, 24)),
    )
}

fn agent_inspector_visible(screen: &str) -> bool {
    screen.contains("A> agent")
        || screen.contains("Practice")
        || screen.contains("Sparky")
        || screen.contains("Agent")
}

fn world_stage_visible(screen: &str) -> bool {
    // The Realm overworld is half-block pixel art; braille remains a fallback.
    screen
        .chars()
        .any(|c| matches!(c, '▀' | '▄') || ('\u{2801}'..='\u{28ff}').contains(&c))
        && screen.lines().any(|line| {
            ((line.contains("[Map]") || line.contains("[Explore]")) && line.contains("[Back]"))
                // A wide enough pane floats the world's actions from its
                // dock: its handle sits at the pane's foot.
                || world_dock_handle(line)
        })
}

fn world_dock_handle(line: &str) -> bool {
    line.contains("│ ▸ ") || line.contains("│ ◂ ")
}

fn has_visual_footer(screen: &str) -> bool {
    screen.lines().any(|line| {
        line.contains('▌')
            && line.contains("[-]")
            && line.contains("[+]")
            && line.contains("[Fit]")
            && line.contains("[Back]")
    })
}

fn run_scenario(rows: u16, cols: u16, resize_to: Option<(u16, u16)>) -> Result<(), String> {
    let scratch = ScratchDir::new(&format!("pty-{cols}x{rows}"))?;
    let mut cockpit = CockpitPty::spawn(rows, cols, &scratch)?;

    let startup = cockpit.wait_for("startup cockpit frame", |screen| {
        screen.contains("angelX") && screen.contains("Write a message")
    })?;
    if startup.contains("ARRIVAL") {
        return Err("fresh startup rendered ARRIVAL instead of Realm".to_string());
    }
    if !cockpit.raw_mode_active()? {
        return Err("cockpit rendered without entering terminal raw mode".to_string());
    }
    // Wide terminals mount the Realm overworld at startup (narrow ones reveal
    // it on focus below). Wait for rendered pixels and working controls;
    // world views intentionally omit the old decorative route headings.
    if cols >= 100 {
        cockpit.wait_for("startup Realm stage", world_stage_visible)?;
    }

    cockpit.send(b"smoke-draft")?;
    cockpit.wait_for("composer draft", |screen| screen.contains("smoke-draft"))?;

    cockpit.send(b"\x1bOR")?; // xterm F3
    cockpit.wait_for("focused Agent inspector", |screen| {
        screen.contains("smoke-draft") && agent_inspector_visible(screen)
    })?;

    cockpit.send(b"\x1bOS")?; // xterm F4
    cockpit.wait_for("focused Realm overworld", |screen| {
        screen.contains("smoke-draft")
            && world_stage_visible(screen)
            && (screen.contains("[Explore]") || screen.lines().any(world_dock_handle))
    })?;

    cockpit.send(b"\x1b")?;
    cockpit.wait_for("idle Stage Back", |screen| {
        screen.contains("smoke-draft") && screen.contains("agent shell")
    })?;

    if let Some((next_rows, next_cols)) = resize_to {
        cockpit.resize(next_rows, next_cols)?;
        cockpit.wait_for("resized cockpit", |screen| {
            screen.contains("smoke-draft") && agent_inspector_visible(screen)
        })?;
    }

    let raw = cockpit.finish()?;
    if find_bytes(&raw, b"[grok]").is_some() {
        return Err("Grok background diagnostics leaked into the interactive terminal".to_string());
    }
    assert_terminal_restored(&raw)
}

fn run_input_latency_scenario(motion: &str, unicode: bool) -> Result<(), String> {
    let draft = if unicode {
        "snappy-0123456789-abcdef-界語-e\u{301}-🦀"
    } else {
        "snappy-input-0123456789-abcdef"
    };
    let guidance_text = if unicode {
        "guided-0123456789-abcdef-界語-e\u{301}-🦀"
    } else {
        "guided"
    };
    const MAX_P95: Duration = Duration::from_millis(25);
    const MAX_BUSY_P95: Duration = Duration::from_millis(25);
    // The budget is for the optimized binary people run (Enter echoes in about
    // 0.3 ms there). A debug build spends ~15-26 ms submitting the first turn
    // with full motion, so it gets its own fixed allowance; qualification runs
    // this smoke with --release against the 25 ms line.
    const MAX_ENTER: Duration = if cfg!(debug_assertions) {
        Duration::from_millis(50)
    } else {
        Duration::from_millis(25)
    };

    let scratch = ScratchDir::new("pty-input-latency")?;
    let mut cockpit = CockpitPty::spawn_with_motion(40, 120, &scratch, &[], motion)?;
    cockpit.wait_for("input latency startup", |screen| {
        screen.contains("angelX") && screen.contains("Write a message")
    })?;

    let mut expected = String::new();
    let mut samples = Vec::with_capacity(draft.len());
    for ch in draft.chars() {
        expected.push(ch);
        let label = format!("composer echo for {expected}");
        let elapsed = cockpit.send_until_visible(
            ch.encode_utf8(&mut [0; 4]).as_bytes(),
            &label,
            |screen| screen.contains(&expected),
        )?;
        samples.push(elapsed);
    }

    eprintln!(
        "PTY_INPUT_RAW motion={motion} phase=idle ms={:?}",
        samples
            .iter()
            .map(|d| d.as_secs_f64() * 1000.0)
            .collect::<Vec<_>>()
    );
    samples.sort_unstable();
    let p50 = samples[(samples.len() - 1) / 2];
    let p95 = samples[(samples.len() - 1) * 95 / 100];
    let worst = *samples.last().unwrap_or(&Duration::ZERO);
    let enter = cockpit.send_until_visible(b"\r", "submitted turn echo", |screen| {
        screen.contains(draft) && screen.contains("Write a message")
    })?;
    cockpit.wait_for("practice turn entered steerable working state", |screen| {
        screen.contains("A> guidance")
    })?;

    let mut guidance = String::new();
    let mut busy_samples = Vec::with_capacity(guidance_text.len());
    for ch in guidance_text.chars() {
        guidance.push(ch);
        let label = format!("busy composer echo for {guidance}");
        let elapsed = cockpit.send_until_visible(
            ch.encode_utf8(&mut [0; 4]).as_bytes(),
            &label,
            |screen| screen.contains(&guidance) && screen.contains("A> guidance"),
        )?;
        busy_samples.push(elapsed);
    }
    eprintln!(
        "PTY_INPUT_RAW motion={motion} phase=busy ms={:?}",
        busy_samples
            .iter()
            .map(|d| d.as_secs_f64() * 1000.0)
            .collect::<Vec<_>>()
    );
    eprintln!(
        "PTY_MOTION_FRAME motion={motion}\n{}",
        cockpit.parser.screen().contents()
    );
    let completion =
        cockpit.wait_for("practice turn completed after busy input probe", |screen| {
            (screen.contains("practice: you said:") && screen.contains(draft))
                || screen.contains("provider request not started:")
        })?;
    if completion.contains("provider request not started:") {
        return Err(format!(
            "practice turn failed before provider dispatch after busy input probe\n{}",
            cockpit.diagnostics()
        ));
    }
    eprintln!(
        "PTY_TERMINAL_WORK motion={motion} captured_bytes={}",
        cockpit.raw.len()
    );

    busy_samples.sort_unstable();
    let busy_p50 = busy_samples[(busy_samples.len() - 1) / 2];
    let busy_p95 = busy_samples[(busy_samples.len() - 1) * 95 / 100];
    let busy_worst = *busy_samples.last().unwrap_or(&Duration::ZERO);
    eprintln!(
        "PTY_INPUT_LATENCY idle_samples={} idle_p50_ms={:.3} idle_p95_ms={:.3} idle_worst_ms={:.3} enter_ms={:.3} busy_samples={} busy_p50_ms={:.3} busy_p95_ms={:.3} busy_worst_ms={:.3}",
        samples.len(),
        p50.as_secs_f64() * 1_000.0,
        p95.as_secs_f64() * 1_000.0,
        worst.as_secs_f64() * 1_000.0,
        enter.as_secs_f64() * 1_000.0,
        busy_samples.len(),
        busy_p50.as_secs_f64() * 1_000.0,
        busy_p95.as_secs_f64() * 1_000.0,
        busy_worst.as_secs_f64() * 1_000.0,
    );
    if p95 > MAX_P95 {
        return Err(format!(
            "composer keypress-to-visible p95 {:.3}ms exceeded {:.3}ms",
            p95.as_secs_f64() * 1_000.0,
            MAX_P95.as_secs_f64() * 1_000.0,
        ));
    }
    if enter > MAX_ENTER {
        return Err(format!(
            "composer Enter-to-visible echo {:.3}ms exceeded {:.3}ms",
            enter.as_secs_f64() * 1_000.0,
            MAX_ENTER.as_secs_f64() * 1_000.0,
        ));
    }
    if busy_p95 > MAX_BUSY_P95 {
        return Err(format!(
            "busy composer keypress-to-visible p95 {:.3}ms exceeded {:.3}ms",
            busy_p95.as_secs_f64() * 1_000.0,
            MAX_BUSY_P95.as_secs_f64() * 1_000.0,
        ));
    }

    let raw = cockpit.finish()?;
    assert_terminal_restored(&raw)
}

fn run_omp_input_latency_scenario(binary: &Path) -> Result<(), String> {
    const DRAFT: &str = "snappy-input-0123456789-abcdef";

    let scratch = ScratchDir::new("pty-omp-input-latency")?;
    let mut cockpit = CockpitPty::spawn_omp(40, 120, &scratch, binary)?;
    let startup = cockpit.wait_for("OMP input latency startup", |screen| {
        screen.contains('π') && screen.contains("work")
    })?;
    if startup.contains("Setup step") {
        return Err("OMP comparator entered its setup wizard".to_string());
    }

    let mut expected = String::new();
    let mut samples = Vec::with_capacity(DRAFT.len());
    for byte in DRAFT.bytes() {
        expected.push(char::from(byte));
        let label = format!("OMP composer echo for {expected}");
        let elapsed =
            cockpit.send_until_visible(&[byte], &label, |screen| screen.contains(&expected))?;
        samples.push(elapsed);
    }

    samples.sort_unstable();
    let p50 = samples[(samples.len() - 1) / 2];
    let p95 = samples[(samples.len() - 1) * 95 / 100];
    let worst = *samples.last().unwrap_or(&Duration::ZERO);
    eprintln!(
        "OMP_PTY_INPUT_LATENCY version={} samples={} p50_ms={:.3} p95_ms={:.3} worst_ms={:.3}",
        std::env::var("ANGEL_OMP_VERSION").unwrap_or_else(|_| "unknown".to_string()),
        samples.len(),
        p50.as_secs_f64() * 1_000.0,
        p95.as_secs_f64() * 1_000.0,
        worst.as_secs_f64() * 1_000.0,
    );

    let _ = cockpit.finish_with(b"\x15\x04", "OMP Ctrl-D")?;
    Ok(())
}

fn run_session_recovery_scenario() -> Result<(), String> {
    const USER_TEXT: &str = "pty-session-recovery-marker";
    const ASSISTANT_TEXT: &str = "practice: you said: pty-session-recovery-marker";

    let scratch = ScratchDir::new("pty-session-recovery")?;
    let mut first = CockpitPty::spawn(40, 120, &scratch)?;
    first.wait_for("first session startup", |screen| {
        screen.contains("angelX") && screen.contains("Write a message")
    })?;
    first.send(format!("{USER_TEXT}\r").as_bytes())?;
    first.wait_for("completed offline practice turn", |screen| {
        screen.contains(USER_TEXT) && screen.contains(ASSISTANT_TEXT)
    })?;
    let snapshot = wait_for_saved_turn(&scratch, USER_TEXT, ASSISTANT_TEXT)?;
    let first_raw = first.finish()?;
    assert_terminal_restored(&first_raw)?;

    let mut resumed = CockpitPty::spawn_with_args(40, 120, &scratch, &["--resume"])?;
    let recovered = resumed.wait_for("resumed conversation frame", |screen| {
        screen.contains(USER_TEXT) && screen.contains(ASSISTANT_TEXT)
    })?;
    if !resumed.raw_mode_active()? {
        return Err("resumed cockpit did not re-enter terminal raw mode".to_string());
    }
    if recovered.contains("session resume failed") {
        return Err(format!(
            "resumed cockpit reported a session failure for {}",
            snapshot.display()
        ));
    }
    let resumed_raw = resumed.finish()?;
    assert_terminal_restored(&resumed_raw)
}

fn run_session_write_failure_scenario() -> Result<(), String> {
    const FIRST_USER: &str = "pty-session-last-good";
    const FIRST_ASSISTANT: &str = "practice: you said: pty-session-last-good";
    const LIVE_USER: &str = "pty-session-memory-only";

    let scratch = ScratchDir::new("pty-session-write-failure")?;
    let mut cockpit = CockpitPty::spawn(40, 120, &scratch)?;
    cockpit.wait_for("session failure startup", |screen| {
        screen.contains("angelX") && screen.contains("Write a message")
    })?;
    let cockpit_pid = cockpit
        .child
        .process_id()
        .ok_or_else(|| "PTY child did not expose a process id".to_string())?;

    cockpit.send(format!("{FIRST_USER}\r").as_bytes())?;
    cockpit.wait_for("last-good practice turn", |screen| {
        screen.contains(FIRST_USER) && screen.contains(FIRST_ASSISTANT)
    })?;
    let snapshot = wait_for_saved_turn(&scratch, FIRST_USER, FIRST_ASSISTANT)?;
    let blocked_tmp = snapshot.with_extension(format!("json.{cockpit_pid}.tmp"));
    fs::create_dir(&blocked_tmp)
        .map_err(|error| format!("block {}: {error}", blocked_tmp.display()))?;

    cockpit.send(format!("{LIVE_USER}\r").as_bytes())?;
    cockpit.wait_for("uncheckpointed turn retained without dispatch", |screen| {
        screen.contains(LIVE_USER)
            && screen.contains("SESSION SAVE DEGRADED")
            && screen.contains("conversation remains in memory")
            && screen.contains("provider request not started")
            && screen.contains("could not durably checkpoint")
    })?;

    let raw_snapshot = fs::read_to_string(&snapshot)
        .map_err(|error| format!("read retained {}: {error}", snapshot.display()))?;
    let record: serde_json::Value = serde_json::from_str(&raw_snapshot)
        .map_err(|error| format!("parse retained {}: {error}", snapshot.display()))?;
    let history = record
        .get("history")
        .and_then(|value| value.as_array())
        .ok_or_else(|| format!("retained snapshot {} has no history", snapshot.display()))?;
    let has = |role: &str, content: &str| {
        history.iter().any(|message| {
            message.get("role").and_then(|value| value.as_str()) == Some(role)
                && message.get("content").and_then(|value| value.as_str()) == Some(content)
        })
    };
    if !has("User", FIRST_USER) || !has("Assistant", FIRST_ASSISTANT) {
        return Err("writer failure damaged the previously published turn".to_string());
    }
    if has("User", LIVE_USER) {
        return Err("failed write unexpectedly replaced the last-good snapshot".to_string());
    }

    // A failed save must not become a clean exit that loses the new input.
    cockpit.send(b"/exit\r")?;
    cockpit.wait_for("exit refused while save is still broken", |screen| {
        screen.contains("/exit: boundary blocked")
    })?;
    if cockpit
        .child
        .try_wait()
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Err("cockpit exited while current user input was not durable".to_string());
    }
    cockpit.send(b"/raw\r")?;
    cockpit.wait_for("unsaved conversation exported", |screen| {
        screen.contains("copy-friendly") && screen.contains("Markdown")
    })?;
    let exported = fs::read_to_string(scratch.path().join("home/.angelX/transcript.txt"))
        .map_err(|error| format!("read unsaved conversation export: {error}"))?;
    if exported.matches(LIVE_USER).count() != 1
        || !exported.contains(FIRST_USER)
        || !exported.contains(FIRST_ASSISTANT)
    {
        return Err("export did not retain exact old and unsaved user content".to_string());
    }
    if fs::read_to_string(&snapshot).map_err(|error| error.to_string())? != raw_snapshot {
        return Err("failed exit/export changed the last-good checkpoint".to_string());
    }
    // Repair only this fixture's injected directory, then retry typed exit.
    fs::remove_dir(&blocked_tmp)
        .map_err(|error| format!("repair owned checkpoint fault: {error}"))?;
    let raw = cockpit.finish()?;
    assert_terminal_restored(&raw)?;
    let repaired: serde_json::Value = serde_json::from_slice(
        &fs::read(&snapshot).map_err(|error| format!("read repaired checkpoint: {error}"))?,
    )
    .map_err(|error| format!("parse repaired checkpoint: {error}"))?;
    let repaired_history = repaired
        .get("history")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "repaired checkpoint has no history".to_string())?;
    for (role, text) in [
        ("User", FIRST_USER),
        ("Assistant", FIRST_ASSISTANT),
        ("User", LIVE_USER),
    ] {
        if repaired_history
            .iter()
            .filter(|message| message["role"] == role && message["content"] == text)
            .count()
            != 1
        {
            return Err(format!(
                "repaired checkpoint must retain {role} {text:?} exactly once"
            ));
        }
    }
    Ok(())
}

fn run_local_image_scenario() -> Result<(), String> {
    let scratch = ScratchDir::new("pty-local-image")?;
    let image = write_visual_fixture(&scratch)?;
    let mut cockpit = CockpitPty::spawn(40, 120, &scratch)?;
    cockpit.wait_for("local image startup", |screen| {
        screen.contains("angelX") && screen.contains("Write a message")
    })?;

    cockpit.send(format!("/show {}\r", image.display()).as_bytes())?;
    cockpit.wait_for("decoded terminal-native image frame", |screen| {
        has_visual_footer(screen)
            && screen.contains("Fit 100%")
            && screen.chars().any(|c| c == '▀' || c == '▄')
    })?;
    let colors = visual_fixture_color_counts(cockpit.parser.screen());
    // The 96×64 source's thin gold diagonal can be averaged away at this
    // terminal geometry. Its broad cyan/dark checker regions must survive.
    if colors.0 < 12 || colors.2 < 12 {
        return Err(format!(
            "decoded terminal-native image frame did not retain fixture colors: cyan={} gold={} dark={}",
            colors.0, colors.1, colors.2
        ));
    }

    cockpit.send(b"\x1b")?;
    cockpit.wait_for("media Back returned to Realm", |screen| {
        !has_visual_footer(screen) && world_stage_visible(screen)
    })?;

    cockpit.send(b"\x1b")?;
    cockpit.wait_for("Realm Back returned to Core", |screen| {
        !has_visual_footer(screen) && screen.contains("agent shell")
    })?;

    let raw = cockpit.finish()?;
    assert_terminal_restored(&raw)
}

fn run_approval_paths_scenario() -> Result<(), String> {
    const COMPOSER_MARKER: &str = "approval-composer-marker";

    let scratch = ScratchDir::new("pty-approval-probe")?;
    let mut cockpit = CockpitPty::spawn(40, 120, &scratch)?;
    cockpit.wait_for("approval probe startup", |screen| {
        screen.contains("angelX") && screen.contains("Write a message")
    })?;

    cockpit.send(b"/approvals probe\r")?;
    cockpit.wait_for("approval probe modal", |screen| {
        screen.contains("Approval needed")
            && screen.contains("approval input probe")
            && screen.contains("y  Approve")
            && screen.contains("n  Deny")
    })?;

    cockpit.send(b"z\x1b")?;
    cockpit.wait_for("approval probe denied", |screen| {
        !screen.contains("Approval needed") && screen.contains("Write a message")
    })?;

    cockpit.send(b"/approvals probe\r")?;
    cockpit.wait_for("approval probe reopened", |screen| {
        screen.contains("Approval needed") && screen.contains("approval input probe")
    })?;
    cockpit.send(b"y")?;
    cockpit.wait_for("approval probe approved", |screen| {
        !screen.contains("Approval needed") && screen.contains("Write a message")
    })?;

    cockpit.send(b"/approvals selftest\r")?;
    cockpit.wait_for("broker self-test modal", |screen| {
        screen.contains("Approval needed")
            && screen.contains("Approval broker self-test")
            && screen.contains("no action will run")
            && screen.contains("SCOPE  action batch")
    })?;
    cockpit.send(b"\x1b")?;
    cockpit.wait_for("broker self-test denied and worker resumed", |screen| {
        !screen.contains("Approval needed")
            && screen.contains("denied")
            && screen.contains("worker resumed")
    })?;

    cockpit.send(b"/approvals selftest\r")?;
    cockpit.wait_for("broker self-test reopened", |screen| {
        screen.contains("Approval needed")
            && screen.contains("Approval broker self-test")
            && screen.contains("SCOPE  action batch")
    })?;
    cockpit.send(b"y")?;
    cockpit.wait_for("broker self-test approved and worker resumed", |screen| {
        !screen.contains("Approval needed")
            && screen.contains("approved")
            && screen.contains("worker resumed")
    })?;

    cockpit.send(COMPOSER_MARKER.as_bytes())?;
    cockpit.wait_for("composer restored after approval", |screen| {
        screen.contains(COMPOSER_MARKER)
    })?;

    let raw = cockpit.finish()?;
    assert_terminal_restored(&raw)
}

fn run_brain_route_controls_scenario() -> Result<(), String> {
    const COMPOSER_MARKER: &str = "route-control-composer-marker";

    let scratch = ScratchDir::new("pty-brain-route-controls")?;
    scratch.provision_codex_route_fixture()?;
    let mut cockpit = CockpitPty::spawn(40, 120, &scratch)?;
    cockpit.wait_for("brain route control startup", |screen| {
        screen.contains("angelX") && screen.contains("Write a message")
    })?;

    cockpit.send(b"/model ChatGPT\r")?;
    cockpit.wait_for("model search by visible connection name", |screen| {
        screen.contains("Brain Route · MODEL")
            && screen.contains("/ChatGPT")
            && screen.contains("pty-sol")
            && !screen.contains("no routes match")
    })?;
    cockpit.send(b"\x7f\x7f\x7f\x7f\x7f\x7f\x7fpty-sol")?;
    cockpit.wait_for("filtered model route deck", |screen| {
        screen.contains("Brain Route · MODEL")
            && screen.contains("/pty-sol")
            && screen.contains("pty-sol")
    })?;
    cockpit.send(b"\r")?;
    cockpit.wait_for("exact model route applied", |screen| {
        !screen.contains("Brain Route · MODEL")
            && (screen.contains("[SET:pty-sol") || screen.contains("[MODEL:pty-sol"))
            && screen.contains("[THINK:low")
    })?;

    cockpit.send(b"/think high\r")?;
    cockpit.wait_for("filtered thinking route deck", |screen| {
        screen.contains("Brain Route · THINK")
            && screen.contains("/high")
            && screen.contains("high")
    })?;
    cockpit.send(b"\r")?;
    cockpit.wait_for("exact reasoning effort applied", |screen| {
        !screen.contains("Brain Route · THINK")
            && (screen.contains("[SET:pty-sol") || screen.contains("[MODEL:pty-sol"))
            && screen.contains("[THINK:high")
    })?;

    cockpit.send(COMPOSER_MARKER.as_bytes())?;
    cockpit.wait_for("composer restored after route controls", |screen| {
        screen.contains(COMPOSER_MARKER)
    })?;

    let raw = cockpit.finish()?;
    assert_terminal_restored(&raw)
}

fn run_live_module_scenario() -> Result<(), String> {
    let scratch = ScratchDir::new("pty-live-module")?;
    let ui = scratch.path().join("live/ui");
    fs::create_dir_all(&ui).map_err(|e| e.to_string())?;
    let shipped_ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("live/ui");
    fs::copy(shipped_ui.join("module.json"), ui.join("module.json")).map_err(|e| e.to_string())?;
    let ui_source = fs::read_to_string(shipped_ui.join("index.js")).map_err(|e| e.to_string())?;
    fs::write(ui.join("index.js"), &ui_source).map_err(|e| e.to_string())?;
    let module = scratch.path().join("live/smoke");
    fs::create_dir_all(&module).map_err(|e| e.to_string())?;
    fs::write(module.join("module.json"),
        r#"{"api_version":1,"id":"smoke","kind":"text","entry":"index.js","probe":{"label":"probe"}}"#)
        .map_err(|e| e.to_string())?;
    fs::write(
        module.join("index.js"),
        "module.exports={apiVersion:1,render: input=>'live-v1-'+input.label};",
    )
    .map_err(|e| e.to_string())?;
    let mut cockpit = CockpitPty::spawn(40, 144, &scratch)?;
    cockpit.wait_for("live host startup", |screen| {
        screen.contains("Write a message")
    })?;
    let pid = cockpit.child.process_id();
    cockpit.send_until_visible(
        b"/live smoke {\"label\":\"first\"}\r",
        "first live generation",
        |screen| screen.contains("live-v1-first"),
    )?;

    let edited = Instant::now();
    fs::write(
        module.join("index.tmp"),
        "module.exports={apiVersion:1,render: input=>'live-v2-'+input.label};",
    )
    .map_err(|e| e.to_string())?;
    fs::rename(module.join("index.tmp"), module.join("index.js")).map_err(|e| e.to_string())?;
    // Allow the watcher and debounce to activate without calling /reload.
    cockpit.drain_for(Duration::from_millis(1500))?;
    cockpit.send_until_visible(
        b"/live smoke {\"label\":\"second\"}\r",
        "automatic live replacement",
        |screen| screen.contains("live-v2-second"),
    )?;
    eprintln!("PTY edit-to-observed replacement: {:?}", edited.elapsed());

    fs::write(module.join("index.js"), "module.exports={").map_err(|e| e.to_string())?;
    cockpit.drain_for(Duration::from_millis(1500))?;
    // A fresh label proves this is a new reply, not the prior rendered row.
    cockpit.send_until_visible(
        b"/live smoke {\"label\":\"third\"}\r",
        "last working code after bad edit",
        |screen| screen.contains("live-v2-third"),
    )?;
    fs::write(
        ui.join("index.js"),
        ui_source.replace("Write a message…", "LIVE COMPOSER EDIT"),
    )
    .map_err(|e| e.to_string())?;
    cockpit.wait_for("live UI repainted without a command", |screen| {
        screen.contains("LIVE COMPOSER EDIT")
    })?;
    fs::write(ui.join("index.js"), "module.exports={").map_err(|e| e.to_string())?;
    cockpit.drain_for(Duration::from_millis(1500))?;
    // Force an input clear/repaint: retained terminal pixels cannot satisfy it.
    cockpit.send(b"temporary draft\x15")?;
    cockpit.wait_for(
        "live UI survived a broken edit and fresh repaint",
        |screen| screen.contains("LIVE COMPOSER EDIT") && !screen.contains("temporary draft"),
    )?;
    if pid != cockpit.child.process_id()
        || cockpit
            .child
            .try_wait()
            .map_err(|e| e.to_string())?
            .is_some()
    {
        return Err("live module changes restarted or exited the native host".into());
    }
    assert_terminal_restored(&cockpit.finish()?)
}

#[test]
fn ordinary_cockpit_real_pty_live_module_replacement_and_bad_edit_smoke() {
    if let Err(error) = run_live_module_scenario() {
        if !strict_pty_smoke() && error.starts_with("open PTY:") {
            eprintln!("PTY_LIVE_MODULE_SKIPPED: {error}");
            return;
        }
        panic!("PTY live module smoke failed: {error}");
    }
}

fn run_live_game_scenario() -> Result<(), String> {
    let scratch = ScratchDir::new("pty-live-game")?;
    let module = scratch.path().join("live/game");
    fs::create_dir_all(&module).map_err(|e| e.to_string())?;
    fs::write(
        module.join("module.json"),
        r#"{"api_version":1,"id":"game","kind":"game","entry":"index.js"}"#,
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        module.join("index.js"),
        "module.exports={apiVersion:1,render:()=>({knobs:{}})}",
    )
    .map_err(|e| e.to_string())?;
    let mut cockpit = CockpitPty::spawn(40, 144, &scratch)?;
    cockpit.wait_for("live game host startup", |screen| {
        screen.contains("Write a message")
    })?;
    let pid = cockpit.child.process_id();
    cockpit.send(b"/dungeon start\r")?;
    cockpit.drain_for(Duration::from_millis(500))?;
    cockpit.send(b"\x1bOQ")?; // F2: keep the run and return to coding.
    cockpit.wait_for("composer beside the running game", |screen| {
        screen.contains("Write a message")
    })?;
    fs::write(
        module.join("index.js"),
        "module.exports={apiVersion:1,render:()=>({knobs:{hero_speed:20,fire_rate:200}})}",
    )
    .map_err(|e| e.to_string())?;
    cockpit.drain_for(Duration::from_millis(1500))?;
    cockpit.send_until_visible(
        b"/dungeon tune hero_speed 10\r",
        "live rules applied to the existing game",
        |screen| screen.contains("currently overrides it to 20"),
    )?;
    fs::write(
        module.join("index.js"),
        "module.exports={apiVersion:1,render:()=>({knobs:{hero_speed:30,unknown_rule:1}})}",
    )
    .map_err(|e| e.to_string())?;
    cockpit.drain_for(Duration::from_millis(1500))?;
    cockpit.send_until_visible(
        b"/dungeon tune hero_speed 12\r",
        "invalid rule set retained the whole working generation",
        |screen| {
            screen.contains("Saved hero_speed = 12")
                && screen.contains("currently overrides it to 20")
        },
    )?;
    fs::write(
        module.join("index.js"),
        "module.exports={apiVersion:1,render:()=>({knobs:{}})}",
    )
    .map_err(|e| e.to_string())?;
    cockpit.drain_for(Duration::from_millis(1500))?;
    cockpit.send_until_visible(
        b"/dungeon tune hero_speed 14\r",
        "removed overlay restored ordinary game tuning",
        |screen| screen.contains("hero_speed = 14 for everyone"),
    )?;
    if cockpit.child.process_id() != pid
        || cockpit
            .child
            .try_wait()
            .map_err(|e| e.to_string())?
            .is_some()
    {
        return Err("live game rules replaced or exited the native host".into());
    }
    assert_terminal_restored(&cockpit.finish()?)
}

#[test]
fn ordinary_cockpit_real_pty_live_game_rules_smoke() {
    if let Err(error) = run_live_game_scenario() {
        if !strict_pty_smoke() && error.starts_with("open PTY:") {
            eprintln!("PTY_LIVE_GAME_SKIPPED: {error}");
            return;
        }
        panic!("PTY live game smoke failed: {error}");
    }
}

fn install_character_modules(scratch: &ScratchDir) -> Result<(), String> {
    for module in ["abilities", "ability-code", "ability-sounds", "ability-effects", "characters"] {
        let destination = scratch.path().join("live").join(module);
        fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("live").join(module)).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            fs::copy(entry.path(), destination.join(entry.file_name())).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires the two native system binaries built by cargo test -p angel-delve-system --features swap-test"]
fn ordinary_cockpit_real_pty_native_system_swap_preserves_the_host_and_run() {
    let result = (|| -> Result<(), String> {
        let scratch = ScratchDir::new("pty-system-swap")?;
        install_character_modules(&scratch)?;
        let baseline = std::env::var("ANGEL_SYSTEM_TEST_WORKER")
            .map_err(|_| "set ANGEL_SYSTEM_TEST_WORKER")?;
        let replacement = std::env::var("ANGEL_SYSTEM_TEST_REPLACEMENT")
            .map_err(|_| "set ANGEL_SYSTEM_TEST_REPLACEMENT")?;
        let engine = scratch.path().join("delve-engine");
        fs::copy(baseline, &engine).map_err(|e| e.to_string())?;
        let mut cockpit = CockpitPty::spawn(40, 144, &scratch)?;
        cockpit.wait_for("system host startup", |s| s.contains("Write a message"))?;
        let pid = cockpit.child.process_id();
        cockpit.send(b"/dungeon start\r")?;
        cockpit.drain_for(Duration::from_millis(500))?;
        cockpit.send(b"\x1bOQ")?;
        cockpit.wait_for("composer beside existing run", |s| {
            s.contains("Write a message")
        })?;
        cockpit.send_until_visible(b"/reload status\r", "initial native system", |s| {
            s.contains("swaps=1") && s.contains("score=0")
        })?;
        cockpit.send_until_visible(b"/character gold 1\r", "character module alongside native rules", |s|s.contains("character gold selected for player 1"))?;
        let pending = engine.with_extension("pending");
        fs::copy(replacement, &pending).map_err(|e| e.to_string())?;
        fs::rename(pending, &engine).map_err(|e| e.to_string())?;
        cockpit.drain_for(Duration::from_millis(900))?;
        cockpit.send_until_visible(
            b"/reload status\r",
            "whole executable replaced between requests",
            |s| s.contains("swaps=2"),
        )?;
        // The replacement's new rule awards seven points every simulation step.
        cockpit.send(b"/dungeon resume\r")?;
        cockpit.drain_for(Duration::from_millis(900))?;
        cockpit.send(b"\x1bOQ")?;
        cockpit.drain_for(Duration::from_millis(200))?;
        cockpit.send_until_visible(
            b"/reload status\r",
            "changed executable behavior on the retained run",
            |s| {
                let last = s.rsplit("Delve system:").next().unwrap_or("");
                last.contains("swaps=2") && last.contains("score=") && !last.contains("score=0")
            },
        )?;
        if cockpit.child.process_id() != pid
            || cockpit
                .child
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_some()
        {
            return Err("system swap replaced/exited the cockpit".into());
        }
        assert_terminal_restored(&cockpit.finish()?)
    })();
    if let Err(error) = result {
        panic!("native system PTY swap failed: {error}");
    }
}

#[test]
fn ordinary_cockpit_real_pty_executable_abilities_keep_in_flight_casts() {
    let result = (|| -> Result<(), String> {
        let scratch = ScratchDir::new("pty-executable-abilities")?;
        install_character_modules(&scratch)?;
        let module = scratch.path().join("live/ability-code");
        let spells = fs::read_to_string(module.join("spells.js")).map_err(|e| e.to_string())?;
        // A long cast stays active while the player returns to coding and
        // changes its executable. Cooldown prevents held-key repeat casts.
        fs::write(
            module.join("spells.js"),
            spells
                .replace("cooldown: 24", "cooldown: 240")
                .replace("delay: 5", "delay: 600"),
        )
        .map_err(|e| e.to_string())?;
        if let Ok(worker) = std::env::var("ANGEL_SYSTEM_TEST_WORKER") {
            fs::copy(worker, scratch.path().join("delve-engine")).map_err(|e| e.to_string())?;
        }
        let mut cockpit = CockpitPty::spawn(40, 144, &scratch)?;
        cockpit.wait_for("ability host startup", |s| s.contains("Write a message"))?;
        let pid = cockpit.child.process_id();
        cockpit.send_until_visible(b"/ability spark\r", "registered ability command", |s| {
            s.contains("ability spark selected for player 1 slot 1")
        })?;
        cockpit.send(b"/dungeon start\r")?;
        cockpit.wait_for_game_view()?;
        cockpit.drain_for(Duration::from_millis(600))?;
        cockpit.send(b"1")?; // Actual spell input through the seat/rules seam.
        cockpit.drain_for(Duration::from_millis(450))?;
        cockpit.send(b"\x1bOQ")?;
        cockpit.wait_for("coding beside active ability", |s| {
            s.contains("Write a message")
        })?;
        cockpit.send_until_visible(b"/reload status\r", "first executable cast", |s| {
            let status = s.rsplit("Delve system:").next().unwrap_or("");
            status.contains("active=1 pending=0 generations=1 started=1 effects=1 faults=0")
                && status.contains("sounds=1")
        })?;
        cockpit.send_until_visible(
            b"/open live.abilities\r",
            "original authored ability view",
            |s| s.contains("Twin Spark"),
        )?;
        let edited = Instant::now();
        let wave = scratch.path().join("live/ability-sounds/spark.wav");
        let mut bytes = fs::read(&wave).map_err(|e| e.to_string())?;
        bytes[80] ^= 0x40;
        fs::write(wave, bytes).map_err(|e| e.to_string())?;
        fs::write(
            module.join("spells.pending"),
            spells
                .replace("Twin Spark", "Prism Burst")
                .replace("damage: 28", "damage: 77")
                .replace("cooldown: 24", "cooldown: 240"),
        )
        .map_err(|e| e.to_string())?;
        fs::rename(module.join("spells.pending"), module.join("spells.js"))
            .map_err(|e| e.to_string())?;
        cockpit.wait_for("new authored ability view", |s| s.contains("Prism Burst"))?;
        eprintln!(
            "PTY executable ability edit-to-observed replacement: {:?}",
            edited.elapsed()
        );
        // Back to chat, with both the game and cast still owned by the host.
        cockpit.send(b"\x1bOQ")?;
        cockpit.drain_for(Duration::from_millis(150))?;
        cockpit.send_until_visible(
            b"/ability spark 1 2\r",
            "new generation selected in another slot",
            |s| s.contains("ability spark selected for player 1 slot 2"),
        )?;
        cockpit.send(b"/dungeon resume\r")?;
        cockpit.wait_for_game_view()?;
        cockpit.drain_for(Duration::from_millis(250))?;
        cockpit.send(b"2")?;
        cockpit.drain_for(Duration::from_millis(600))?;
        cockpit.send(b"\x1bOQ")?;
        cockpit.wait_for("coding after new ability", |s| {
            s.contains("Write a message")
        })?;
        cockpit.send_until_visible(
            b"/reload status\r",
            "new cast finished while original timeline remains",
            |s| {
                let status = s.rsplit("Delve system:").next().unwrap_or("");
                status.contains("active=1 pending=0 generations=1 started=2 effects=3 faults=0")
                    && status.contains("sounds=3")
            },
        )?;
        cockpit.send_until_visible(b"/ability cancel 1\r", "explicit cast cancellation", |s| {
            s.contains("cancelled module abilities for player 1")
        })?;
        cockpit.send_until_visible(b"/reload status\r", "retired executable generations", |s| {
            let status = s.rsplit("Delve system:").next().unwrap_or("");
            status.contains("active=0 pending=0 generations=0 started=2 effects=3 faults=0")
                && status.contains("sounds=3")
        })?;
        if cockpit.child.process_id() != pid
            || cockpit
                .child
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_some()
        {
            return Err("ability replacement restarted/exited the cockpit".into());
        }
        let raw = cockpit.finish()?;
        if let Some(pid) = pid {
            let prefix = format!("angel-live-sound-{pid}-");
            let leaked = fs::read_dir(std::env::temp_dir())
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().starts_with(&prefix));
            if leaked {
                return Err("sound files survived normal host shutdown".into());
            }
        }
        assert_terminal_restored(&raw)
    })();
    if let Err(error) = result {
        if !strict_pty_smoke() && error.starts_with("open PTY:") {
            return;
        }
        panic!("executable ability PTY failed: {error}");
    }
}

#[test]
fn ordinary_cockpit_real_pty_resource_dependencies_rebind_an_open_surface_atomically() {
    let result = (|| -> Result<(), String> {
        let scratch = ScratchDir::new("pty-resource-dependencies")?;
        for (id, version) in [("ink", 1), ("fabric", 2)] {
            let module = scratch.path().join("live").join(id);
            fs::create_dir_all(&module).map_err(|e| e.to_string())?;
            fs::write(module.join("module.json"),format!(r#"{{"api_version":2,"id":"{id}","kind":"system","entry":"index.js","state_schema":1,"exports":[{{"kind":"command","name":"{id}","contract":"command/1"}},{{"kind":"resource","name":"pack","contract":"pack/1"}}]}}"#)).map_err(|e|e.to_string())?;
            fs::write(module.join("index.js"),format!("module.exports={{apiVersion:2,initialize:()=>({{}}),reduce:state=>({{state}}),resources:()=>({{pack:{{version:{version}}}}})}}")).map_err(|e|e.to_string())?;
        }
        let consumer = scratch.path().join("live/a-choice");
        fs::create_dir_all(&consumer).map_err(|e| e.to_string())?;
        fs::write(consumer.join("module.json"),r#"{"api_version":2,"id":"a-choice","kind":"system","entry":"index.js","state_schema":1,"imports":[{"module":"ink","contract":"pack/1"},{"module":"fabric","contract":"pack/1"}],"exports":[{"kind":"command","name":"outfit","contract":"command/1"},{"kind":"resource","name":"pack","contract":"choice/1"},{"kind":"surface","name":"choice","contract":"surface/1"}]}"#).map_err(|e|e.to_string())?;
        fs::write(consumer.join("index.js"),r#"const ink=resource('ink','pack'),fabric=resource('fabric','pack');module.exports={apiVersion:2,initialize:()=>({selected:'ordinary'}),migrate:p=>({...p.state,selected:'MIGRATION RERAN'}),reduce:(state,event)=>({state:{selected:event.arguments},output:'selection '+event.arguments}),resources:state=>{if(ink.value.version===99)throw Error('incompatible ink selection');return {pack:{ink:ink.value.version,fabric:fabric.value.version,selected:state.selected}}},view:state=>({choice:{title:'Dependent resource view',lines:[`ink ${ink.value.version} fabric ${fabric.value.version} selected ${state.selected}`]}})}"#).map_err(|e|e.to_string())?;
        let mut cockpit = CockpitPty::spawn(40, 144, &scratch)?;
        cockpit.wait_for("resource host startup", |s| s.contains("Write a message"))?;
        let pid = cockpit.child.process_id();
        cockpit.send_until_visible(b"/outfit chosen\r", "persistent choice", |s| {
            s.contains("selection chosen")
        })?;
        cockpit.send_until_visible(b"/open live.choice\r", "initial dependency view", |s| {
            s.contains("ink 1 fabric 2 selected chosen")
        })?;
        let source = scratch.path().join("live/ink/index.js");
        let original = fs::read_to_string(&source).map_err(|e| e.to_string())?;
        let edited = Instant::now();
        fs::write(&source, original.replace("version:1", "version:3"))
            .map_err(|e| e.to_string())?;
        cockpit.wait_for("rebound open view without changing consumer source", |s| {
            s.contains("ink 3 fabric 2 selected chosen")
        })?;
        eprintln!(
            "PTY independent resource edit-to-view: {:?}",
            edited.elapsed()
        );
        fs::write(&source, original.replace("version:1", "version:99"))
            .map_err(|e| e.to_string())?;
        cockpit.drain_for(Duration::from_millis(450))?;
        cockpit.send(b"\x1bOQ")?;
        cockpit.wait_for("coding after incompatible resource", |s| {
            s.contains("Write a message")
        })?;
        cockpit.send_until_visible(b"/reload\r", "rejected dependency candidate", |s| {
            s.contains("incompatible ink selection") && s.contains("a-choice · generation 2")
        })?;
        cockpit.send_until_visible(
            b"/open live.choice\r",
            "retained coherent dependency view",
            |s| s.contains("ink 3 fabric 2 selected chosen"),
        )?;
        if cockpit.child.process_id() != pid
            || cockpit
                .child
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_some()
        {
            return Err("resource rebind replaced the host".into());
        }
        assert_terminal_restored(&cockpit.finish()?)
    })();
    if let Err(error) = result {
        if !strict_pty_smoke() && error.starts_with("open PTY:") {
            return;
        }
        panic!("resource dependency PTY failed: {error}");
    }
}

#[test]
fn ordinary_cockpit_real_pty_visual_swap_keeps_effects_after_cast_completion() {
    let result = (|| -> Result<(), String> {
        let scratch = ScratchDir::new("pty-module-vfx")?;
        install_character_modules(&scratch)?;
        let module = scratch.path().join("live/ability-effects");
        let spells = fs::read_to_string(scratch.path().join("live/ability-code/spells.js")).map_err(|e| e.to_string())?;
        fs::write(
            scratch.path().join("live/ability-code/spells.js"),
            spells
                .replace("delay: 5", "delay: 0")
                .replace("cooldown: 24", "cooldown: 240"),
        )
        .map_err(|e| e.to_string())?;
        let effects = fs::read_to_string(module.join("effects.js"))
            .map_err(|e| e.to_string())?
            .replace("ticks: 90", "ticks: 600")
            .replace("key(90,", "key(600,");
        fs::write(module.join("effects.js"), &effects).map_err(|e| e.to_string())?;
        if let Ok(worker) = std::env::var("ANGEL_SYSTEM_TEST_WORKER") {
            fs::copy(worker, scratch.path().join("delve-engine")).map_err(|e| e.to_string())?;
        }
        let mut cockpit = CockpitPty::spawn(40, 144, &scratch)?;
        cockpit.wait_for("VFX host startup", |s| s.contains("Write a message"))?;
        let pid = cockpit.child.process_id();
        cockpit.send_until_visible(b"/ability spark\r", "select one-shot ability", |s| {
            s.contains("ability spark selected for player 1 slot 1")
        })?;
        cockpit.send(b"/dungeon start\r")?;
        cockpit.wait_for_game_view()?;
        cockpit.drain_for(Duration::from_millis(600))?;
        cockpit.send(b"1")?;
        cockpit.drain_for(Duration::from_millis(300))?;
        cockpit.send(b"\x1bOQ")?;
        cockpit.wait_for("coding beside finished cast", |s| {
            s.contains("Write a message")
        })?;
        cockpit.send_until_visible(b"/reload status\r", "effect outlives its cast", |s| {
            let status = s
                .rsplit("Delve system:")
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect::<String>();
            status.contains("active=0pending=0generations=0started=1effects=1faults=0")
                && status.contains("vfx=1vfx-active=1vfx-generations=1")
        })?;
        cockpit.send_until_visible(
            b"/open live.abilities\r",
            "original effect description",
            |s| s.contains("Crystal sigils"),
        )?;
        let edited = Instant::now();
        image::RgbaImage::from_pixel(32, 32, image::Rgba([255, 40, 80, 255]))
            .save(module.join("sigil.png"))
            .map_err(|e| e.to_string())?;
        fs::write(
            module.join("effects.pending"),
            effects.replace("Crystal sigils", "Ember sigils"),
        )
        .map_err(|e| e.to_string())?;
        fs::rename(module.join("effects.pending"), module.join("effects.js"))
            .map_err(|e| e.to_string())?;
        cockpit.wait_for("new effect description without a reload", |s| {
            s.contains("Ember sigils")
        })?;
        eprintln!(
            "PTY effect source/asset edit-to-observed activation: {:?}",
            edited.elapsed()
        );
        cockpit.send(b"\x1bOQ")?;
        cockpit.drain_for(Duration::from_millis(100))?;
        cockpit.send_until_visible(
            b"/ability spark 1 2\r",
            "new effect generation selected",
            |s| s.contains("ability spark selected for player 1 slot 2"),
        )?;
        cockpit.send(b"/dungeon resume\r")?;
        cockpit.wait_for_game_view()?;
        cockpit.drain_for(Duration::from_millis(250))?;
        cockpit.send(b"2")?;
        cockpit.drain_for(Duration::from_millis(300))?;
        cockpit.send(b"\x1bOQ")?;
        cockpit.wait_for("coding beside both effects", |s| {
            s.contains("Write a message")
        })?;
        cockpit.send_until_visible(
            b"/reload status\r",
            "old and new VFX live independently of finished casts",
            |s| {
                let status = s
                    .rsplit("Delve system:")
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .collect::<String>();
                status.contains("active=0pending=0generations=0started=2effects=2faults=0")
                    && status.contains("vfx=2vfx-active=2vfx-generations=2")
            },
        )?;
        cockpit.send_until_visible(
            b"/ability cancel 1\r",
            "explicit effect cancellation",
            |s| s.contains("cancelled module abilities for player 1"),
        )?;
        cockpit.send_until_visible(b"/reload status\r", "retired visual resources", |s| {
            let status = s
                .rsplit("Delve system:")
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect::<String>();
            status.contains("vfx=2vfx-active=0vfx-generations=0")
        })?;
        if cockpit.child.process_id() != pid
            || cockpit
                .child
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_some()
        {
            return Err("effect replacement restarted/exited the cockpit".into());
        }
        assert_terminal_restored(&cockpit.finish()?)
    })();
    if let Err(error) = result {
        if !strict_pty_smoke() && error.starts_with("open PTY:") {
            return;
        }
        panic!("module VFX PTY failed: {error}");
    }
}

#[test]
fn ordinary_cockpit_real_pty_lifecycle_and_focus_smoke() {
    let strict = strict_pty_smoke();
    for (rows, cols, resize_to) in [(24, 80, Some((40, 120))), (40, 120, None)] {
        if let Err(error) = run_scenario(rows, cols, resize_to) {
            if !strict && error.starts_with("open PTY:") {
                eprintln!("PTY_SMOKE_SKIPPED {cols}x{rows}: {error}");
                return;
            }
            panic!("PTY smoke failed at {cols}x{rows}: {error}");
        }
    }
}

fn run_research_workspace_scenario() -> Result<(), String> {
    let scratch = ScratchDir::new("pty-research-workspace")?;
    let mut cockpit = CockpitPty::spawn(40, 144, &scratch)?;
    // The animated Realm world map is the startup stage; the research
    // workspace is an explicit destination reached through `/research`.
    cockpit.wait_for("realm startup", |screen| {
        world_stage_visible(screen) && screen.contains("Write a message")
    })?;
    cockpit.send(b"/research\r")?;
    cockpit.wait_for("research command", |screen| {
        screen.contains("Keep / Campaign overview")
    })?;
    cockpit.send(b"z")?;
    cockpit.wait_for("expanded research directory", |screen| {
        screen.contains("REALM / RESEARCH PLACES")
    })?;
    cockpit.send(b"2")?;
    cockpit.wait_for("research ledger", |screen| {
        screen.contains("OPERATION") && screen.contains("OBSERVATION")
    })?;
    cockpit.send(b"3")?;
    cockpit.wait_for("research flow", |screen| {
        screen.contains("OBSERVED WORK") && screen.contains("RECORDED OUTCOME")
    })?;
    cockpit.send(b"\x1b[C")?;
    cockpit.wait_for("Round Table research place", |screen| {
        screen.contains("Round Table / Agents & handoffs")
    })?;
    cockpit.send(b"w")?;
    cockpit.wait_for("full Realm", |screen| {
        world_stage_visible(screen) && !screen.contains("REALM / RESEARCH PLACES")
    })?;
    cockpit.send(b"r")?;
    cockpit.wait_for("return from Realm", |screen| {
        screen.contains("Keep / Campaign overview")
    })?;
    cockpit.resize(24, 80)?;
    cockpit.wait_for("compact research layout", |screen| {
        // The old wide frame's heading survives a VT grid resize. Wait for
        // both bottom regions, which appear only after the cockpit repaints
        // at the new height, before sending the next draft.
        screen.contains("Keep / Campaign overview")
            && screen.contains("3 Flow")
            && screen.contains("[Inspect]")
            && screen.contains("Write a message")
    })?;
    cockpit.send(b"/goal draft-123")?;
    cockpit.wait_for("research preserves composer draft", |screen| {
        screen.contains("/goal draft-123")
    })?;
    let raw = cockpit.finish()?;
    assert_terminal_restored(&raw)
}

#[test]
fn ordinary_cockpit_real_pty_research_workspace_smoke() {
    for _ in 0..12 {
        if let Err(error) = run_research_workspace_scenario() {
            if !strict_pty_smoke() && error.starts_with("open PTY:") {
                eprintln!("PTY_RESEARCH_WORKSPACE_SKIPPED: {error}");
                return;
            }
            panic!("PTY research workspace smoke failed: {error}");
        }
    }
}

#[test]
fn ordinary_cockpit_real_pty_excalibur_startup_and_first_draft() {
    let scratch = ScratchDir::new("pty-excalibur").unwrap();
    // A configured, entirely fake route avoids the practice warning occupying
    // the transcript. Never submit a model turn; finish is the local exit command.
    scratch.provision_codex_route_fixture().unwrap();
    let mut cockpit =
        CockpitPty::spawn_with_driver(32, 80, &scratch, &[], "reduced", "openai").unwrap();
    cockpit
        .wait_for("empty cockpit", |screen| screen.contains("Write a message"))
        .unwrap();
    // Reduced motion selects the final sword pose immediately. Inspect actual
    // emitted terminal cells, not the atlas or an in-memory render fixture.
    cockpit
        .wait_for("Braille sword in empty shell", |screen| {
            screen
                .lines()
                .skip(5)
                .take(18)
                .flat_map(|line| line.chars().skip(3).take(73))
                .filter(|c| ('\u{2801}'..='\u{28ff}').contains(c))
                .count()
                > 10
        })
        .unwrap();
    let sword_ink = |screen: &vt100::Screen| {
        (5..23)
            .flat_map(|y| (3..76).map(move |x| (y, x)))
            .filter(|&(y, x)| {
                screen.cell(y, x).is_some_and(|cell| {
                    cell.fgcolor() == vt100::Color::Rgb(255, 255, 255)
                        && cell.bgcolor() == vt100::Color::Default
                        && cell
                            .contents()
                            .chars()
                            .any(|c| ('\u{2801}'..='\u{28ff}').contains(&c))
                })
            })
            .count()
    };
    // Other Dotmax ambience can arrive before the intro worker. Wait for this
    // surface's pure-white, transparent ink rather than accepting an unrelated
    // Braille glyph or the former steel-on-black presentation.
    let deadline = Instant::now() + STEP_TIMEOUT;
    while sword_ink(cockpit.parser.screen()) <= 10 {
        assert!(
            Instant::now() < deadline,
            "Excalibur dots never arrived: {}",
            cockpit.diagnostics()
        );
        cockpit.drain_for(Duration::from_millis(20)).unwrap();
    }
    cockpit
        .send_until_visible(b"sword-draft", "first draft", |screen| {
            screen.contains("sword-draft")
        })
        .unwrap();
    cockpit.drain_for(Duration::from_millis(900)).unwrap();
    assert_eq!(sword_ink(cockpit.parser.screen()), 0, "sword did not fade");
    cockpit.send(b"\x15").unwrap();
    cockpit
        .wait_for("draft erased", |screen| screen.contains("Write a message"))
        .unwrap();
    cockpit.drain_for(Duration::from_millis(300)).unwrap();
    assert_eq!(
        sword_ink(cockpit.parser.screen()),
        0,
        "erasing draft replayed intro"
    );
    let raw = cockpit.finish().unwrap();
    assert_terminal_restored(&raw).unwrap();
}

#[test]
fn ordinary_cockpit_real_pty_input_latency_smoke() {
    if let Err(error) = run_input_latency_scenario("off", false) {
        if !strict_pty_smoke() && error.starts_with("open PTY:") {
            eprintln!("PTY_INPUT_LATENCY_SKIPPED: {error}");
            return;
        }
        panic!("PTY input latency smoke failed: {error}");
    }
}

#[test]
fn ordinary_cockpit_real_pty_motion_unicode_latency_smoke() {
    let mut errors = Vec::new();
    for motion in ["full", "reduced", "off"] {
        if let Err(error) = run_input_latency_scenario(motion, true) {
            errors.push(format!("{motion}: {error}"));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[cfg(target_os = "linux")]
#[test]
fn ordinary_cockpit_real_pty_terminal_work_probe() {
    for motion in ["full", "reduced", "off"] {
        let scratch = ScratchDir::new("pty-terminal-work").unwrap();
        let mut cockpit = CockpitPty::spawn_with_motion(40, 120, &scratch, &[], motion).unwrap();
        cockpit
            .wait_for("work probe startup", |screen| {
                screen.contains("Write a message")
            })
            .unwrap();
        cockpit.drain_for(Duration::from_millis(150)).unwrap();
        for phase in ["idle", "active"] {
            if phase == "active" {
                cockpit.send(b"terminal-work-probe\r").unwrap();
                cockpit
                    .wait_for("practice busy", |screen| screen.contains("A> guidance"))
                    .unwrap();
            }
            let ticks = cockpit.cpu_ticks().unwrap();
            let bytes = cockpit.raw.len();
            let started = Instant::now();
            cockpit.drain_for(Duration::from_millis(400)).unwrap();
            let elapsed = started.elapsed().as_secs_f64();
            let ticks = cockpit.cpu_ticks().unwrap().saturating_sub(ticks);
            let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
            assert!(hz > 0);
            eprintln!(
                "PTY_STEADY_WORK motion={motion} phase={phase} rows=40 cols=120 wall_secs={elapsed:.6} cpu_ticks={ticks} clock_ticks_per_sec={hz} cpu_one_core_pct={:.3} output_bytes={}",
                ticks as f64 / hz as f64 / elapsed * 100.0,
                cockpit.raw.len() - bytes
            );
        }
        let raw = cockpit.finish().unwrap();
        assert_terminal_restored(&raw).unwrap();
    }
}

#[test]
fn omp_comparator_real_pty_input_latency_probe() {
    let Some(binary) = std::env::var_os("ANGEL_OMP_BIN") else {
        eprintln!("OMP_PTY_INPUT_LATENCY_SKIPPED: set ANGEL_OMP_BIN to opt in");
        return;
    };
    let binary = PathBuf::from(binary);
    if let Err(error) = run_omp_input_latency_scenario(&binary) {
        panic!("OMP comparator input latency probe failed: {error}");
    }
}

#[test]
fn ordinary_cockpit_real_pty_session_recovery_smoke() {
    if let Err(error) = run_session_recovery_scenario() {
        if !strict_pty_smoke() && error.starts_with("open PTY:") {
            eprintln!("PTY_SESSION_RECOVERY_SKIPPED: {error}");
            return;
        }
        panic!("PTY session recovery smoke failed: {error}");
    }
}

#[test]
fn ordinary_cockpit_real_pty_session_write_failure_smoke() {
    if let Err(error) = run_session_write_failure_scenario() {
        if !strict_pty_smoke() && error.starts_with("open PTY:") {
            eprintln!("PTY_SESSION_WRITE_FAILURE_SKIPPED: {error}");
            return;
        }
        panic!("PTY session write failure smoke failed: {error}");
    }
}

#[test]
fn ordinary_cockpit_real_pty_local_image_smoke() {
    if let Err(error) = run_local_image_scenario() {
        if !strict_pty_smoke() && error.starts_with("open PTY:") {
            eprintln!("PTY_LOCAL_IMAGE_SKIPPED: {error}");
            return;
        }
        panic!("PTY local image smoke failed: {error}");
    }
}

#[test]
fn ordinary_cockpit_real_pty_approval_paths_smoke() {
    if let Err(error) = run_approval_paths_scenario() {
        if !strict_pty_smoke() && error.starts_with("open PTY:") {
            eprintln!("PTY_APPROVAL_PROBE_SKIPPED: {error}");
            return;
        }
        panic!("PTY approval probe smoke failed: {error}");
    }
}

#[test]
fn ordinary_cockpit_real_pty_brain_route_controls_smoke() {
    if let Err(error) = run_brain_route_controls_scenario() {
        if !strict_pty_smoke() && error.starts_with("open PTY:") {
            eprintln!("PTY_BRAIN_ROUTE_CONTROLS_SKIPPED: {error}");
            return;
        }
        panic!("PTY Brain Route controls smoke failed: {error}");
    }
}

#[test]
fn ordinary_cockpit_registers_stateful_command_and_surface_then_swaps_them_live() {
    let result=(|| -> Result<(),String> {
        let scratch=ScratchDir::new("pty-stateful-system")?;
        fs::create_dir_all(scratch.path().join("live")).map_err(|e|e.to_string())?;
        let mut cockpit=CockpitPty::spawn(40,144,&scratch)?;
        cockpit.wait_for("kernel host startup",|s|s.contains("Write a message"))?;
        let pid=cockpit.child.process_id();
        // Install a completely new command and surface after the host booted.
        let module=scratch.path().join("live/tally");
        fs::create_dir_all(&module).map_err(|e|e.to_string())?;
        let manifest=|schema|format!(r#"{{"api_version":2,"id":"tally","kind":"system","entry":"index.js","state_schema":{schema},"exports":[{{"kind":"command","name":"tally","contract":"command/1"}},{{"kind":"surface","name":"tally","contract":"surface/1"}}]}}"#);
        fs::write(module.join("module.json"),manifest(1)).map_err(|e|e.to_string())?;
        fs::write(module.join("index.js"),"module.exports={apiVersion:2,initialize:()=>({n:0}),reduce:(s,e)=>({state:{n:s.n+1},output:'TALLY-FIRST-'+(s.n+1)}),view:s=>({tally:{title:'Tally module',lines:['LIVE-SURFACE-FIRST-'+s.n]}})}").map_err(|e|e.to_string())?;
        cockpit.drain_for(Duration::from_millis(1500))?;
        cockpit.send_until_visible(b"/tally\r","registered command",|s|s.contains("TALLY-FIRST-1"))?;
        cockpit.send_until_visible(b"/open live.tally\r","registered Glass surface",|s|s.contains("LIVE-SURFACE-FIRST-1"))?;
        // Change the state shape and executable reducer. The old count survives.
        let edited=Instant::now();
        fs::write(module.join("next.js"),"module.exports={apiVersion:2,migrate:p=>({total:p.state.n}),reduce:(s,e)=>({state:{total:s.total+10},output:'TALLY-SECOND-'+(s.total+10)}),view:s=>({tally:{title:'Migrated tally',lines:['LIVE-SURFACE-SECOND-'+s.total]}})}").map_err(|e|e.to_string())?;
        fs::rename(module.join("next.js"),module.join("index.js")).map_err(|e|e.to_string())?;
        fs::write(module.join("module.json"),manifest(2)).map_err(|e|e.to_string())?;
        cockpit.wait_for("surface replaced while it stayed open",|s|s.contains("LIVE-SURFACE-SECOND-1"))?;
        eprintln!("PTY stateful system edit-to-paint: {:?}",edited.elapsed());
        cockpit.send(b"/tally\r")?;
        cockpit.wait_for("new reducer on migrated state",|s|s.contains("LIVE-SURFACE-SECOND-11"))?;
        fs::write(module.join("index.js"),"module.exports={apiVersion:2,migrate:()=>{throw Error('bad migration')},reduce:()=>{}}").map_err(|e|e.to_string())?;
        cockpit.drain_for(Duration::from_millis(1500))?;
        cockpit.send(b"/tally\r")?;
        cockpit.wait_for("failed migration retained state and code",|s|s.contains("LIVE-SURFACE-SECOND-21"))?;
        cockpit.send(b"/close live.tally\r")?;
        cockpit.send(b"\x1bOQ")?;
        cockpit.wait_for("returned to coding",|s|s.contains("Write a message"))?;
        if pid!=cockpit.child.process_id() || cockpit.child.try_wait().map_err(|e|e.to_string())?.is_some() {return Err("module transaction restarted the host".into());}
        assert_terminal_restored(&cockpit.finish()?)
    })();
    if let Err(error)=result {
        if !strict_pty_smoke() && error.starts_with("open PTY:") {eprintln!("PTY_SYSTEM_SKIPPED: {error}");return;}
        panic!("PTY stateful module smoke failed: {error}");
    }
}

#[test]
#[ignore = "requires the pinned optional Bend developer compiler; run explicitly"]
fn ordinary_cockpit_real_pty_fairy_field_controls_enemies_and_keeps_old_fields_on_swap() {
    let result = (|| -> Result<(), String> {
        let scratch = ScratchDir::new("pty-fairy-field")?;
        install_character_modules(&scratch)?;
        // Resume a real combat save, so a field must stop actual enemy AI.
        let mut run = angel_delve::rules::Run::new(3, 9, None);
        let room = run
            .dungeon
            .rooms
            .iter()
            .position(|room| room.kind == angel_delve::rules::RoomKind::Fight)
            .unwrap();
        run.enter_for_test(room);
        run.calm_for_test();
        run.enemies.clear();
        let floor = &mut run.dungeon.rooms[room];
        for y in 1..floor.rows - 1 {
            for x in 1..floor.cols - 1 {
                floor.set(x, y, angel_delve::rules::Tile::Floor);
            }
        }
        let hero = run.players.get_mut(&1).unwrap();
        hero.x = 8.0;
        hero.y = 8.0;
        run.spawn_at_for_test(angel_delve::rules::EnemyKind::Skeleton, 10.0, 8.0);
        run.spawn_at_for_test(angel_delve::rules::EnemyKind::Skeleton, 14.0, 8.0);
        let save = scratch.path().join("work/.angel/dungeon/session.json");
        fs::create_dir_all(save.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(
            save,
            serde_json::to_vec(
                &serde_json::json!({"version":1,"raid_serial":9,"run":run,"invitation":null}),
            )
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if let Ok(worker) = std::env::var("ANGEL_SYSTEM_TEST_WORKER") {
            fs::copy(worker, scratch.path().join("delve-engine")).map_err(|e| e.to_string())?;
        }
        let mut cockpit = CockpitPty::spawn(40, 144, &scratch)?;
        cockpit.wait_for("field host startup", |s| s.contains("Write a message"))?;
        let pid = cockpit.child.process_id();
        cockpit.send_until_visible(b"/ability fairies\r", "registered fairy skill", |s| {
            s.contains("ability fairies selected for player 1 slot 1")
        })?;
        cockpit.send(b"/dungeon resume\r")?;
        cockpit.wait_for_game_view()?;
        cockpit.drain_for(Duration::from_millis(300))?;
        cockpit.send(b"1")?;
        cockpit.drain_for(Duration::from_millis(170))?;
        cockpit.send(b"\x1bOQ")?;
        cockpit.wait_for("coding beside first fairy field", |s| {
            s.contains("Write a message")
        })?;
        cockpit.send_until_visible(b"/reload status\r", "nearby enemy actually stunned", |s| {
            let status = s
                .rsplit("Delve system:")
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect::<String>();
            status.contains("active=1pending=0generations=1started=1")
                && status.contains("faults=0")
                && status.contains("controlled-enemies=1")
        })?;
        cockpit.send_until_visible(
            b"/open live.abilities\r",
            "original fairy definition",
            |s| s.contains("Fairy Stun Field"),
        )?;
        let source = scratch.path().join("live/ability-code/fairies.bend");
        let before = fs::read_to_string(&source).map_err(|e| e.to_string())?;
        let edited = Instant::now();
        fs::write(
            &source,
            before
                .replace("Fairy Stun Field", "Wide Fairy Field")
                .replace("Params{3.5, 150, 12, 12}", "Params{7.0, 150, 12, 12}"),
        )
        .map_err(|e| e.to_string())?;
        cockpit.wait_for("new field module without restart", |s| {
            s.contains("Wide Fairy Field")
        })?;
        eprintln!("PTY fairy field edit-to-activation: {:?}", edited.elapsed());
        cockpit.send(b"\x1bOQ")?;
        cockpit.drain_for(Duration::from_millis(100))?;
        cockpit.send_until_visible(b"/ability fairies 1 2\r", "new field selected", |s| {
            s.contains("ability fairies selected for player 1 slot 2")
        })?;
        cockpit.send(b"/dungeon resume\r")?;
        cockpit.wait_for_game_view()?;
        cockpit.drain_for(Duration::from_millis(150))?;
        cockpit.send(b"2")?;
        cockpit.drain_for(Duration::from_millis(170))?;
        cockpit.send(b"\x1bOQ")?;
        cockpit.wait_for("coding beside two field generations", |s| {
            s.contains("Write a message")
        })?;
        cockpit.send_until_visible(
            b"/reload status\r",
            "wider new field controls the second enemy",
            |s| {
                let status = s
                    .rsplit("Delve system:")
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .collect::<String>();
                status.contains("active=2pending=0generations=2started=2")
                    && status.contains("faults=0")
                    && status.contains("controlled-enemies=2")
                    && status.contains("vfx-active=2")
            },
        )?;
        cockpit.send_until_visible(b"/ability cancel 1\r", "cancel both fields", |s| {
            s.contains("cancelled module abilities for player 1")
        })?;
        cockpit.send_until_visible(
            b"/reload status\r",
            "field actors and visual resources retired",
            |s| {
                let status = s
                    .rsplit("Delve system:")
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .collect::<String>();
                status.contains("active=0pending=0generations=0started=2")
                    && status.contains("vfx-active=0")
            },
        )?;
        if cockpit.child.process_id() != pid
            || cockpit
                .child
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_some()
        {
            return Err("fairy field swap replaced/exited the cockpit".into());
        }
        assert_terminal_restored(&cockpit.finish()?)
    })();
    if let Err(error) = result {
        panic!("fairy field PTY failed: {error}");
    }
}

#[test]
#[ignore = "requires the two native system binaries built with swap-test"]
fn ordinary_cockpit_real_pty_pending_realm_claim_survives_a_native_rules_swap() {
    let result = (|| -> Result<(), String> {
        use angel_delve::realm::{Haul, Spoil, Spoils};
        use angel_delve::rules::{Run, home::Home, host_commands::Journal};
        let scratch = ScratchDir::new("pty-realm-claim-swap")?;
        let worker = std::env::var("ANGEL_SYSTEM_TEST_WORKER")
            .map_err(|_| "set ANGEL_SYSTEM_TEST_WORKER")?;
        let replacement = std::env::var("ANGEL_SYSTEM_TEST_REPLACEMENT")
            .map_err(|_| "set ANGEL_SYSTEM_TEST_REPLACEMENT")?;
        let engine = scratch.path().join("delve-engine");
        fs::copy(worker, &engine).map_err(|e| e.to_string())?;
        let mut run = Run::at_home(3, 9, None, Home::default(), Spoils::default());
        let mut gold = Spoils::default();
        gold.add(Spoil::Gold, 19);
        run.bank.push(Haul {
            hero: 1,
            spoils: gold,
            why: "earned before the implementation swap".into(),
        });
        assert!(run.valid_snapshot());
        let save = scratch.path().join("work/.angel/dungeon/session.json");
        fs::create_dir_all(save.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(
            &save,
            serde_json::to_vec(
                &serde_json::json!({"version":1,"raid_serial":9,"run":run,"invitation":null}),
            )
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        // Block only the destination store; the rest of normal startup remains
        // available. The game must durably retain its claim before trying it.
        let directory = scratch.path().join("home/.angelX/world-rewards");
        fs::create_dir_all(directory.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&directory, b"treasury storage is unavailable").map_err(|e| e.to_string())?;
        let journal = || -> Result<Journal, String> {
            let saved: serde_json::Value =
                serde_json::from_slice(&fs::read(&save).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if saved["version"] != 6 || saved["run"]["raid_id"] != 9 {
                return Err("missing current private checkpoint".into());
            }
            let session = saved["commands"]["session"]
                .as_str()
                .ok_or("missing saved session")?
                .to_owned();
            Journal::restore(saved["commands"].clone(), &session, 9)
        };
        let mut cockpit = CockpitPty::spawn(40, 144, &scratch)?;
        cockpit.wait_for("claim host startup", |s| s.contains("Write a message"))?;
        let pid = cockpit.child.process_id();
        cockpit.wait_for("retained claim saved before destination payment", |_| {
            journal()
                .ok()
                .and_then(|j| j.pending_realm().ok().flatten())
                .is_some()
        })?;
        let claim = journal()?
            .pending_realm()?
            .ok_or("no retained realm claim")?;
        if claim.transfer.hauls.len() != 1 || claim.transfer.hauls[0].spoils.get(Spoil::Gold) != 19
        {
            return Err("bank claim did not retain its original payload".into());
        }
        cockpit.send_until_visible(b"/reload status\r", "original native rules", |s| {
            s.contains("swaps=1")
        })?;
        let staged = engine.with_extension("pending");
        fs::copy(replacement, &staged).map_err(|e| e.to_string())?;
        let edited = Instant::now();
        fs::rename(staged, &engine).map_err(|e| e.to_string())?;
        cockpit.drain_for(Duration::from_millis(900))?;
        cockpit.send_until_visible(
            b"/reload status\r",
            "native replacement with an unresolved claim",
            |s| s.contains("swaps=2"),
        )?;
        eprintln!(
            "PTY pending-claim rules swap observed: {:?}",
            edited.elapsed()
        );
        if journal()?.pending_realm()? != Some(claim.clone()) {
            return Err("rules replacement changed the pending claim identity/payload".into());
        }
        fs::remove_file(&directory).map_err(|e| e.to_string())?;
        fs::create_dir(&directory).map_err(|e| e.to_string())?;
        let economy = || -> Option<serde_json::Value> {
            let path = fs::read_dir(&directory)
                .ok()?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.ends_with(".realm.json"))
                })?;
            serde_json::from_slice(&fs::read(path).ok()?).ok()
        };
        cockpit.wait_for(
            "original claim paid and acknowledged after recovery",
            |_| {
                let paid = economy().is_some_and(|value| {
                    value["treasury"]["gold"] == 19
                        && value["settlements"][&claim.session]["request"] == claim.request
                });
                let acknowledged = journal().is_ok_and(|j| {
                    j.session() == claim.session
                        && j.pending_realm().is_ok_and(|claim| claim.is_none())
                });
                paid && acknowledged
            },
        )?;
        cockpit.send(b"/dungeon off\r")?;
        cockpit.wait_for("game close after settlement", |s| {
            s.contains("Dungeon closed")
        })?;
        if economy().ok_or("realm disappeared")?["treasury"]["gold"] != 19 {
            return Err("close replayed the original reward".into());
        }
        if pid != cockpit.child.process_id()
            || cockpit
                .child
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_some()
        {
            return Err("claim recovery restarted/exited the host".into());
        }
        assert_terminal_restored(&cockpit.finish()?)
    })();
    if let Err(error) = result {
        panic!("PTY realm-claim swap failed: {error}");
    }
}

#[test]
#[ignore = "requires the two native system binaries built with swap-test"]
fn ordinary_cockpit_real_pty_pending_home_purchase_survives_a_native_rules_swap() {
    let result = (|| -> Result<(), String> {
        use angel_delve::realm::{Spoil, Spoils};
        use angel_delve::rules::{
            Run,
            home::{Home, Order, Station},
            host_commands::Journal,
            home_settlement::{HomeOutbox, settle},
        };
        let scratch = ScratchDir::new("pty-home-claim-swap")?;
        let worker = std::env::var("ANGEL_SYSTEM_TEST_WORKER")
            .map_err(|_| "set ANGEL_SYSTEM_TEST_WORKER")?;
        let replacement = std::env::var("ANGEL_SYSTEM_TEST_REPLACEMENT")
            .map_err(|_| "set ANGEL_SYSTEM_TEST_REPLACEMENT")?;
        let engine = scratch.path().join("delve-engine");
        fs::copy(worker, &engine).map_err(|e| e.to_string())?;
        let mut run = Run::at_home(3, 9, None, Home::default(), Spoils::default());
        run.stall = vec!["potion".into()];
        run.orders.push(Order {
            knight: 1,
            station: Station::StallA,
        });
        run.marks.insert("xp".into(), 2);
        run.marks.insert("native_policy_probe".into(), 1);
        run.marks.insert("country:fireflies".into(), 1);
        run.marks.insert("country:song".into(), 1);
        run.feats.push("roadkill");
        run.dungeon.rooms[run.at].items.clear();
        for hero in run.players.values_mut() {
            hero.hand.retain(|id| id != "potion");
            hero.deck.retain(|id| id != "potion");
        }
        let mut journal = Journal::new("pty-home-session".into(), 9);
        let claim = journal
            .claim_home(&mut run)?
            .ok_or("missing initial home claim")?;
        let mut home = Home::default();
        let mut treasury = Spoils::default();
        let expected = settle(
            &mut angel_delve::rules::activity::Realm {
                home: &mut home,
                treasury: &mut treasury,
                reclaimed: &Default::default(),
                raids_won: 0,
            },
            &claim.transfer,
            "Host",
            false,
        )?;
        if expected.sales != [(0, 1)] || expected.treasury.get(Spoil::Gold) == 0 {
            return Err("home fixture did not earn and buy its reserved item".into());
        }
        // The replacement executable grants seven gold for this explicit fact.
        // A host-linked settlement policy would retain the old balance and fail.
        let expected_gold = expected.treasury.get(Spoil::Gold) + 7;
        let save = scratch.path().join("work/.angel/dungeon/session.json");
        fs::create_dir_all(save.parent().unwrap()).map_err(|e| e.to_string())?;
        let country: std::collections::BTreeMap<_, _> = run
            .marks
            .iter()
            .filter(|(key, _)| angel_delve::rules::countryside::intent_key(key))
            .map(|(key, value)| (key.clone(), *value))
            .collect();
        fs::write(
            &save,
            serde_json::to_vec(&serde_json::json!({
                "version":3,"raid_serial":9,"run":run,"invitation":null,
                "country_intents":country,"home_outbox":HomeOutbox::capture(&run)?,
                "commands":journal.capture()?
            }))
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let directory = scratch.path().join("home/.angelX/world-rewards");
        fs::create_dir_all(directory.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&directory, b"treasury storage is unavailable").map_err(|e| e.to_string())?;
        let checkpoint = || -> Result<(serde_json::Value, Journal), String> {
            let saved: serde_json::Value =
                serde_json::from_slice(&fs::read(&save).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let journal = Journal::restore(saved["commands"].clone(), &claim.session, 9)?;
            Ok((saved, journal))
        };
        let mut cockpit = CockpitPty::spawn(40, 144, &scratch)?;
        cockpit.wait_for("home claim host startup", |s| s.contains("Write a message"))?;
        let pid = cockpit.child.process_id();
        cockpit.wait_for("host rewrote durable pending home facts", |_| {
            checkpoint().is_ok_and(|(saved, journal)| {
                saved.get("realm_triumph").is_some()
                    && journal.pending_home().ok().flatten() == Some(claim.clone())
                    && saved["home_outbox"]["marks"]["xp"] == 2
                    && saved["home_outbox"]["orders"]
                        .as_array()
                        .is_some_and(|orders| orders.len() == 1)
            })
        })?;
        cockpit.send_until_visible(b"/reload status\r", "original home rules worker", |s| {
            s.contains("swaps=1")
        })?;
        let staged = engine.with_extension("pending");
        fs::copy(replacement, &staged).map_err(|e| e.to_string())?;
        let edited = Instant::now();
        fs::rename(staged, &engine).map_err(|e| e.to_string())?;
        cockpit.drain_for(Duration::from_millis(900))?;
        cockpit.send_until_visible(
            b"/reload status\r",
            "replacement retains pending home claim",
            |s| s.contains("swaps=2"),
        )?;
        eprintln!(
            "PTY pending-home rules swap observed: {:?}",
            edited.elapsed()
        );
        let (saved, pending) = checkpoint()?;
        if pending.pending_home()? != Some(claim.clone())
            || saved["home_outbox"]["marks"]["xp"] != 2
        {
            return Err("worker swap lost or changed pending home facts".into());
        }
        fs::remove_file(&directory).map_err(|e| e.to_string())?;
        fs::create_dir(&directory).map_err(|e| e.to_string())?;
        let economy = || -> Option<serde_json::Value> {
            let path = fs::read_dir(&directory)
                .ok()?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.ends_with(".realm.json"))
                })?;
            serde_json::from_slice(&fs::read(path).ok()?).ok()
        };
        let key = format!("{}/home", claim.session);
        cockpit.wait_for("home reward and purchase committed once", |_| {
            let paid = economy().is_some_and(|value| {
                value["treasury"]["gold"] == expected_gold
                    && value["home"]["countryside"]["fireflies_paid"] == true
                    && value["settlements"][&key]["request"] == claim.request
                    && value["settlements"][&key]["home"]["delivery_fingerprint"].is_string()
                    && value["settlements"][&key]["home"]["result"].is_null()
            });
            let delivered = checkpoint().is_ok_and(|(saved, journal)| {
                let item = saved["run"]["dungeon"]["rooms"]
                    .as_array()
                    .is_some_and(|rooms| {
                        rooms.iter().any(|room| {
                            room["items"].as_array().is_some_and(|items| {
                                items.iter().any(|item| item["card"] == "potion")
                            })
                        })
                    });
                let held = saved["run"]["players"].as_object().is_some_and(|players| {
                    players.values().any(|hero| {
                        ["hand", "deck"].iter().any(|field| {
                            hero[field]
                                .as_array()
                                .is_some_and(|cards| cards.iter().any(|id| id == "potion"))
                        })
                    })
                });
                journal
                    .pending_home()
                    .is_ok_and(|pending| pending.is_none())
                    && saved["home_outbox"]["orders"] == serde_json::json!([])
                    && saved["home_outbox"]["marks"]
                        .as_object()
                        .is_some_and(|marks| marks.is_empty())
                    && (item || held)
            });
            paid && delivered
        })?;
        cockpit.send(b"/dungeon off\r")?;
        cockpit.wait_for("close after the home receipt", |s| {
            s.contains("Dungeon closed")
        })?;
        if economy().ok_or("home realm disappeared")?["treasury"]["gold"] != expected_gold {
            return Err("close replayed a home reward or purchase".into());
        }
        if pid != cockpit.child.process_id()
            || cockpit
                .child
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_some()
        {
            return Err("home claim recovery restarted/exited the host".into());
        }
        assert_terminal_restored(&cockpit.finish()?)
    })();
    if let Err(error) = result {
        panic!("PTY home-claim swap failed: {error}");
    }
}
