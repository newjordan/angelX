//! Test delegation for the swarm.
//!
//! Designated "delegator" agents (the empiricist / red-team personas) can ask to
//! *verify a claim* by running a test or consulting a specialist model. They emit
//! a `swarm-test` block; angel auto-approves it against a policy and routes it by
//! the agent's per-request hint:
//! - **local**  — run in a scratch workspace; default-on `ANGEL_SANDBOX` arms the
//!   cockpit's [`sandbox`](crate::agent::sandbox), writes are landlock-confined to it.
//! - **remote** — ssh to a configured host (`ANGEL_SWARM_SSH_<HOST>`) and run there.
//! - **peer**   — hand the claim to another network agent for a second opinion
//!   (best-effort, via a configurable mq9/robusty command).
//! - **phone**  — consult a model API (the "sota phone"): dial a specific model
//!   (`local` is the local box) or a category alias (math/fast) for a hard sub-problem. Reuses
//!   [`HttpClub`](crate::agent::club::HttpClub), so any OpenAI-compatible endpoint works.
//!
//! The approval policy is the safety boundary: for command placements, a denylist
//! (no `sudo`, `rm`, command-substitution, pipe-to-shell, exfil tools…) plus a
//! program allowlist, stricter for remote. Phone skips the command policy (it
//! sends text to a model, it doesn't execute) but is gated by `ANGEL_SWARM_PHONE`.
//! Off by default; enabled per run with `ANGEL_SWARM_DELEGATE` (folded into
//! `ANGEL_SWARM_MAX`); the non-local placements each need their own opt-in.

use crate::agent::club::HttpClub;
use crate::agent::sandbox::{self, SandboxPolicy};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub enum Placement {
    Local,
    Remote(String),
    Peer(String),
    Phone(String),
}

impl Placement {
    pub fn label(&self) -> String {
        match self {
            Placement::Local => "local".to_string(),
            Placement::Remote(h) => format!("remote:{h}"),
            Placement::Peer(a) => format!("peer:{a}"),
            Placement::Phone(m) => format!("phone:{m}"),
        }
    }

    pub fn approval_scope(&self) -> crate::agent::approval::ApprovalScope {
        match self {
            Placement::Local => crate::agent::approval::ApprovalScope::SelfTest,
            Placement::Remote(host) => {
                crate::agent::approval::ApprovalScope::RemoteHost(host.clone())
            }
            Placement::Peer(peer) => crate::agent::approval::ApprovalScope::Peer(peer.clone()),
            Placement::Phone(model) => {
                crate::agent::approval::ApprovalScope::PhoneModel(model.clone())
            }
        }
    }

    pub fn needs_approval(&self) -> bool {
        !matches!(self, Placement::Local)
    }
}

#[derive(Clone, Debug)]
pub struct TestRequest {
    pub claim: String,
    pub cmd: String,
    pub placement: Placement,
    pub why: String,
}

pub struct TestResult {
    pub request: TestRequest,
    pub verdict: &'static str,
    pub detail: String,
}

const FENCE: &str = "```swarm-test";

pub fn parse_requests(text: &str) -> Vec<TestRequest> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(FENCE) {
        let after = &rest[start + FENCE.len()..];
        let Some(end) = after.find("```") else { break };
        if let Some(req) = parse_block(&after[..end]) {
            out.push(req);
        }
        rest = &after[end + 3..];
    }
    out
}

pub fn strip_blocks(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find(FENCE) {
        out.push_str(&rest[..start]);
        let after = &rest[start + FENCE.len()..];
        match after.find("```") {
            Some(end) => rest = &after[end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

pub fn dedupe_requests(requests: Vec<TestRequest>, max: usize) -> Vec<TestRequest> {
    if max == 0 {
        return Vec::new();
    }
    let mut seen = std::collections::HashSet::new();
    let mut unique = Vec::with_capacity(requests.len().min(max));
    for req in requests {
        let key = (
            req.placement.label(),
            req.cmd.trim().to_string(),
            req.claim.trim().to_string(),
        );
        if seen.insert(key) {
            unique.push(req);
            if unique.len() == max {
                break;
            }
        }
    }
    unique
}

fn parse_block(body: &str) -> Option<TestRequest> {
    let (mut where_, mut cmd, mut claim, mut why) = (
        String::from("local"),
        String::new(),
        String::new(),
        String::new(),
    );
    for line in body.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("where:") {
            where_ = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("cmd:") {
            cmd = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("claim:") {
            claim = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("why:") {
            why = v.trim().to_string();
        }
    }
    let placement = parse_placement(&where_)?;
    if cmd.is_empty() && (!matches!(&placement, Placement::Phone(_)) || claim.trim().is_empty()) {
        return None;
    }
    Some(TestRequest {
        claim,
        cmd,
        placement,
        why,
    })
}

fn parse_placement(s: &str) -> Option<Placement> {
    let s = s.split('#').next().unwrap_or(s).trim();
    if let Some(h) = s.strip_prefix("remote:") {
        let h = h.trim();
        (!h.is_empty()).then(|| Placement::Remote(h.to_string()))
    } else if let Some(a) = s.strip_prefix("peer:") {
        let a = a.trim();
        (!a.is_empty()).then(|| Placement::Peer(a.to_string()))
    } else if let Some(m) = s.strip_prefix("phone:") {
        Some(Placement::Phone(m.trim().to_string()))
    } else if s == "local" {
        Some(Placement::Local)
    } else {
        None
    }
}

const LOCAL_ALLOW: &[&str] = &[
    "cargo",
    "rustc",
    "python3",
    "python",
    "pytest",
    "node",
    "npm",
    "go",
    "gcc",
    "g++",
    "clang",
    "nvcc",
    "make",
    "ls",
    "cat",
    "echo",
    "true",
    "head",
    "tail",
    "wc",
    "grep",
    "rg",
    "find",
    "test",
    "sort",
    "uniq",
    "diff",
    "sed",
    "awk",
    "jq",
    "stat",
    "file",
    "env",
    "printf",
    "date",
    "nvidia-smi",
    "nproc",
    "uname",
];

const REMOTE_ALLOW: &[&str] = &[
    "cargo",
    "rustc",
    "python3",
    "python",
    "pytest",
    "nvcc",
    "nvidia-smi",
    "ls",
    "cat",
    "echo",
    "nproc",
    "uname",
    "free",
    "df",
    "go",
    "make",
    "true",
];

const BAD_PROGRAMS: &[&str] = &[
    "rm",
    "rmdir",
    "dd",
    "mkfs",
    "shutdown",
    "reboot",
    "halt",
    "poweroff",
    "kill",
    "pkill",
    "killall",
    "chmod",
    "chown",
    "mv",
    "cp",
    "scp",
    "sftp",
    "rsync",
    "nc",
    "ncat",
    "telnet",
    "mount",
    "umount",
    "systemctl",
    "service",
    "crontab",
    "iptables",
    "ufw",
    "passwd",
    "chsh",
    "useradd",
    "userdel",
    "fdisk",
    "parted",
    "tee",
    "truncate",
];

const BAD_PATTERNS: &[&str] = &[
    "sudo",
    ":(){",
    "/dev/sd",
    "/dev/nvme",
    "/etc/passwd",
    "/etc/shadow",
    ".ssh",
    "$(",
    "`",
    "| sh",
    "|sh",
    "| bash",
    "|bash",
    "curl ",
    "wget ",
    " > /",
    ">/",
    " >> /",
];

pub struct Router {
    workspace: PathBuf,
    remote_enabled: bool,
    peer_enabled: bool,
    phone_enabled: bool,
    peer_cmd: String,
    timeout: Duration,
}

impl Router {
    pub fn from_env() -> Self {
        let workspace = std::env::var("ANGEL_SWARM_TEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("angel-swarm-tests"));
        let secs = std::env::var("ANGEL_SWARM_TEST_TIMEOUT")
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(60u64);
        Self {
            workspace,
            remote_enabled: truthy_env("ANGEL_SWARM_REMOTE"),
            peer_enabled: truthy_env("ANGEL_SWARM_PEER"),
            phone_enabled: truthy_env("ANGEL_SWARM_PHONE"),
            peer_cmd: std::env::var("ANGEL_SWARM_PEER_CMD").unwrap_or_else(|_| {
                "robusty-mq9-request send \"$SWARM_PEER_AGENT\" \"$SWARM_PEER_PROMPT\"".to_string()
            }),
            timeout: Duration::from_secs(secs.max(1)),
        }
    }

    fn approve(&self, req: &TestRequest) -> Result<(), String> {
        if let Placement::Phone(target) = &req.placement {
            if !self.phone_enabled {
                return Err("phone disabled (set ANGEL_SWARM_PHONE=1)".to_string());
            }
            if phone_target(target).is_none() {
                let up = target.to_ascii_uppercase().replace('-', "_");
                return Err(format!(
                    "unknown phone target '{target}' (set ANGEL_PHONE_{up}_URL)"
                ));
            }
            return Ok(());
        }
        if let Some(bad) = dangerous(&req.cmd) {
            return Err(format!("blocked by policy: '{bad}'"));
        }
        match &req.placement {
            Placement::Local => {
                if programs_allowed(&req.cmd, LOCAL_ALLOW) {
                    Ok(())
                } else {
                    Err("program not in the local allowlist".to_string())
                }
            }
            Placement::Remote(host) => {
                if !self.remote_enabled {
                    return Err("remote execution disabled (set ANGEL_SWARM_REMOTE=1)".to_string());
                }
                if ssh_target(host).is_none() {
                    let up = host.to_ascii_uppercase().replace('-', "_");
                    return Err(format!(
                        "no ssh target for '{host}' (set ANGEL_SWARM_SSH_{up})"
                    ));
                }
                if uses_inline_interpreter(&req.cmd) {
                    return Err(
                        "inline interpreter code is not allowed for remote execution".to_string(),
                    );
                }
                if programs_allowed(&req.cmd, REMOTE_ALLOW) {
                    Ok(())
                } else {
                    Err("program not allowed for remote execution".to_string())
                }
            }
            Placement::Peer(_) => {
                if self.peer_enabled {
                    Ok(())
                } else {
                    Err("peer second-opinion disabled (set ANGEL_SWARM_PEER=1)".to_string())
                }
            }
            Placement::Phone(_) => unreachable!("phone handled above"),
        }
    }

    pub fn run(&self, req: &TestRequest) -> TestResult {
        if let Err(reason) = self.approve(req) {
            return TestResult {
                request: req.clone(),
                verdict: "denied",
                detail: reason,
            };
        }
        match &req.placement {
            Placement::Local => self.run_local(req),
            Placement::Remote(host) => self.run_remote(req, host),
            Placement::Peer(agent) => self.run_peer(req, agent),
            Placement::Phone(target) => self.run_phone(req, target),
        }
    }

    fn run_local(&self, req: &TestRequest) -> TestResult {
        if let Err(e) = std::fs::create_dir_all(&self.workspace) {
            return self.errored(req, format!("create workspace: {e}"));
        }
        let policy = SandboxPolicy {
            writable_roots: vec![self.workspace.clone(), PathBuf::from("/tmp")],
            allow_network: true,
            enforce: sandbox::enabled(),
            mandatory: false,
            sealed_reads: Vec::new(),
            deny_reads: Vec::new(),
        };
        let mut cmd = match sandbox::command("sh", ["-c", req.cmd.as_str()], &policy) {
            Ok(command) => command,
            Err(error) => return self.errored(req, format!("prepare sandbox: {error}")),
        };
        cmd.current_dir(&self.workspace);
        self.finish(req, cmd, "")
    }

    fn run_remote(&self, req: &TestRequest, host: &str) -> TestResult {
        let Some(target) = ssh_target(host) else {
            return self.errored(req, format!("no ssh target for '{host}'"));
        };
        let mut cmd = Command::new("ssh");
        cmd.arg("-o")
            .arg("BatchMode=yes")
            .arg("-o")
            .arg("ConnectTimeout=8")
            .arg(&target)
            .arg(&req.cmd);
        self.finish(req, cmd, &format!("[ssh {target}] "))
    }

    fn run_peer(&self, req: &TestRequest, agent: &str) -> TestResult {
        use crate::agent::harness::book::{connect::recite, d3_roles::pages, d5_frames};
        let page = |n: usize| recite(&pages(d5_frames::CONSULTS, [n]));
        let prompt = format!(
            "{} {} {}\n{} {}\n{}",
            page(1),
            page(2),
            req.claim,
            page(3),
            req.cmd,
            page(4)
        );
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(&self.peer_cmd)
            .env("SWARM_PEER_AGENT", agent)
            .env("SWARM_PEER_PROMPT", prompt);
        match run_capture(cmd, self.timeout) {
            Ok((out, timed)) => TestResult {
                request: req.clone(),
                verdict: if timed {
                    "timeout"
                } else if out.status.success() {
                    "second-opinion"
                } else {
                    "error"
                },
                detail: fmt_output(&out, timed, ""),
            },
            Err(e) => self.errored(req, format!("peer transport failed: {e}")),
        }
    }

    fn run_phone(&self, req: &TestRequest, target: &str) -> TestResult {
        let Some((url, model, key)) = phone_target(target) else {
            return self.errored(req, format!("unknown phone target '{target}'"));
        };
        let question = if req.cmd.trim().is_empty() {
            req.claim.as_str()
        } else {
            req.cmd.as_str()
        };
        use crate::agent::harness::book::{connect, d3_roles::pages, d5_frames::CONSULTS};
        let context = if req.claim.trim().is_empty() {
            String::new()
        } else {
            format!("{} {}\n\n", pages(CONSULTS, [7]), req.claim)
        };
        let prompt = format!(
            "{}\n\n{context}{} {question}",
            pages(CONSULTS, [5, 6]),
            pages(CONSULTS, [8])
        );
        let club = HttpClub::new(format!("phone:{target}"), url, model, key);
        match connect::respond(&club, &connect::workspace(), &prompt) {
            Ok(ans) if !ans.trim().is_empty() => TestResult {
                request: req.clone(),
                verdict: "phoned",
                detail: ans,
            },
            Ok(_) => self.errored(req, "phone returned an empty answer".to_string()),
            Err(e) => self.errored(req, format!("phone failed: {e}")),
        }
    }

    fn finish(&self, req: &TestRequest, cmd: Command, prefix: &str) -> TestResult {
        match run_capture(cmd, self.timeout) {
            Ok((out, timed)) => TestResult {
                request: req.clone(),
                verdict: if timed {
                    "timeout"
                } else if out.status.success() {
                    "pass"
                } else {
                    "fail"
                },
                detail: fmt_output(&out, timed, prefix),
            },
            Err(e) => self.errored(req, e),
        }
    }

    fn errored(&self, req: &TestRequest, detail: String) -> TestResult {
        TestResult {
            request: req.clone(),
            verdict: "error",
            detail,
        }
    }
}

pub fn evidence_block(results: &[TestResult]) -> String {
    let mut s = format!(
        "{}\n",
        crate::agent::harness::book::d5_frames::EVIDENCE.cells()
    );
    let unstated = crate::agent::harness::book::d5_frames::UNSTATED_CLAIM.cells();
    for r in results {
        let claim = if r.request.claim.is_empty() {
            unstated.as_str()
        } else {
            &r.request.claim
        };
        s.push_str(&format!(
            "\n- [{} · {}] `{}`\n  claim: {}\n  result: {}\n",
            r.request.placement.label(),
            r.verdict,
            r.request.cmd,
            claim,
            truncate(&r.detail, 1200)
        ));
        if !r.request.why.is_empty() {
            s.push_str(&format!("  rationale: {}\n", r.request.why));
        }
    }
    s
}

fn programs_allowed(cmd: &str, allow: &[&str]) -> bool {
    let Some(segments) = shell_segments(cmd) else {
        return false;
    };
    !segments.is_empty()
        && segments.iter().all(|words| {
            command_position(words)
                .and_then(|i| words.get(i))
                .map(|w| allow.contains(&basename(w)))
                .unwrap_or(false)
        })
}

fn command_position(words: &[String]) -> Option<usize> {
    let mut i = 0;
    while words.get(i).is_some_and(|w| is_assignment(w)) {
        i += 1;
    }
    if words.get(i).is_some_and(|w| basename(w) == "env") {
        i += 1;
        if words.get(i).is_some_and(|w| w.starts_with('-')) {
            return None;
        }
        while words.get(i).is_some_and(|w| is_assignment(w)) {
            i += 1;
        }
    }
    words.get(i).map(|_| i)
}

fn uses_inline_interpreter(cmd: &str) -> bool {
    shell_segments(cmd).is_some_and(|segments| {
        segments.iter().any(|words| {
            let Some(i) = command_position(words) else {
                return false;
            };
            let program = basename(&words[i]);
            let args = &words[i + 1..];
            matches!(program, "python" | "python3")
                && args
                    .iter()
                    .any(|arg| arg == "-c" || (arg.starts_with("-c") && arg.len() > 2))
        })
    })
}

fn basename(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

fn shell_segments(cmd: &str) -> Option<Vec<Vec<String>>> {
    if cmd.contains('`') || cmd.contains("$(") {
        return None;
    }
    let mut segments = Vec::new();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;

    let finish_word = |word: &mut String, words: &mut Vec<String>| {
        if !word.is_empty() {
            words.push(std::mem::take(word));
        }
    };
    let finish_segment =
        |word: &mut String, words: &mut Vec<String>, segments: &mut Vec<Vec<String>>| {
            finish_word(word, words);
            if !words.is_empty() {
                segments.push(std::mem::take(words));
            }
        };

    let mut chars = cmd.chars().peekable();
    while let Some(c) = chars.next() {
        if escaped {
            word.push(c);
            escaped = false;
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else if c == '\\' && q == '"' {
                escaped = true;
            } else {
                word.push(c);
            }
            continue;
        }
        match c {
            '\\' => escaped = true,
            '\'' | '"' => quote = Some(c),
            c if c.is_whitespace() && c != '\n' => finish_word(&mut word, &mut words),
            ';' | '\n' | '(' | ')' => finish_segment(&mut word, &mut words, &mut segments),
            '|' => {
                if chars.peek() == Some(&'|') {
                    chars.next();
                }
                finish_segment(&mut word, &mut words, &mut segments);
            }
            '&' if word.ends_with('>') || word.ends_with('<') => word.push(c),
            '&' if chars.peek() == Some(&'&') => {
                chars.next();
                finish_segment(&mut word, &mut words, &mut segments);
            }
            '&' => return None,
            _ => word.push(c),
        }
    }
    if quote.is_some() || escaped {
        return None;
    }
    finish_segment(&mut word, &mut words, &mut segments);
    Some(segments)
}

fn dangerous(cmd: &str) -> Option<String> {
    let lc = cmd.to_ascii_lowercase();
    for pat in BAD_PATTERNS {
        if lc.contains(pat) {
            return Some((*pat).to_string());
        }
    }
    for tok in lc.split(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '|' | '(' | ')')) {
        let tok = tok.rsplit('/').next().unwrap_or(tok);
        if BAD_PROGRAMS.contains(&tok) {
            return Some(tok.to_string());
        }
    }
    None
}

fn ssh_target(host: &str) -> Option<String> {
    let up = host.to_ascii_uppercase().replace('-', "_");
    std::env::var(format!("ANGEL_SWARM_SSH_{up}"))
        .ok()
        .map(|target| target.trim().to_string())
        .filter(|target| !target.is_empty())
}

fn phone_target(key: &str) -> Option<(String, String, Option<String>)> {
    let key = match key.trim() {
        "" | "default" | "sota" | "deep" | "ds" | "math" => "deepseek",
        "flash" | "fast" | "quick" => "deepseek-flash",
        k => k,
    };
    let up = key.to_ascii_uppercase().replace('-', "_");
    let builtin: Option<(Option<&str>, &str, Option<&str>)> = match key {
        "deepseek" => Some((
            Some("https://api.deepseek.com/v1"),
            "deepseek-v4-pro",
            Some("DEEPSEEK_API_KEY"),
        )),
        "deepseek-flash" => Some((
            Some("https://api.deepseek.com/v1"),
            "deepseek-flash",
            Some("DEEPSEEK_API_KEY"),
        )),
        "local" => Some((None, "", None)),
        _ => None,
    };
    let url = std::env::var(format!("ANGEL_PHONE_{up}_URL"))
        .ok()
        .or_else(|| std::env::var(format!("ANGEL_{up}_URL")).ok())
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
        .or_else(|| builtin.and_then(|(url, _, _)| url.map(str::to_string)))?;
    let model = std::env::var(format!("ANGEL_PHONE_{up}_MODEL"))
        .ok()
        .or_else(|| std::env::var(format!("ANGEL_{up}_MODEL")).ok())
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
        .or_else(|| builtin.map(|(_, m, _)| m.to_string()))
        .unwrap_or_else(|| key.to_string());
    let api_key = std::env::var(format!("ANGEL_PHONE_{up}_KEY"))
        .ok()
        .or_else(|| {
            builtin
                .and_then(|(_, _, ke)| ke)
                .and_then(|k| std::env::var(k).ok())
        })
        .or_else(|| std::env::var("ANGEL_BRAIN_KEY").ok());
    Some((url, model, api_key))
}

fn fmt_output(out: &std::process::Output, timed: bool, prefix: &str) -> String {
    let mut s = String::from(prefix);
    s.push_str(String::from_utf8_lossy(&out.stdout).trim());
    let e = String::from_utf8_lossy(&out.stderr);
    if !e.trim().is_empty() {
        s.push_str("\n[stderr] ");
        s.push_str(e.trim());
    }
    if timed {
        s.push_str("\n[timed out]");
    }
    let code = out.status.code().unwrap_or(-1);
    format!("(exit {code}) {}", s.trim())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…[truncated]")
    }
}

fn run_capture(cmd: Command, timeout: Duration) -> Result<(std::process::Output, bool), String> {
    let capture = crate::agent::harness::exec::output_timed_captured(cmd, Some(timeout))?;
    if capture.timed_out {
        return Ok((capture.output, true));
    }
    if capture.cancelled {
        return Err(format!(
            "command cancelled: {}",
            fmt_output(&capture.output, false, "")
        ));
    }
    if !capture.output_complete() {
        return Err(format!(
            "command output capture incomplete: {}",
            fmt_output(&capture.output, false, "")
        ));
    }
    Ok((capture.output, false))
}

fn truthy_env(key: &str) -> bool {
    match std::env::var(key) {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        }
        Err(_) => false,
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/swarm_delegate__tests.rs"]
mod tests;
