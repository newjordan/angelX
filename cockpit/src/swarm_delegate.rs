//! Test delegation for the swarm.
//!
//! Designated "delegator" agents (the empiricist / red-team personas) can ask to
//! *verify a claim* by running a test or consulting a specialist model. They emit
//! a `swarm-test` block; angel auto-approves it against a policy and routes it by
//! the agent's per-request hint:
//! - **local**  — run in a scratch workspace; default-on `ANGEL_SANDBOX` arms the
//!   cockpit's [`sandbox`](crate::sandbox), writes are landlock-confined to it.
//! - **remote** — ssh to a fleet host (spark/atlas/turbo) and run there.
//! - **peer**   — hand the claim to another network agent for a second opinion
//!   (best-effort, via a configurable mq9/robusty command).
//! - **phone**  — consult a model API (the "sota phone"): dial a specific model
//!   or a category alias (math/code/reason/fast) for a hard sub-problem. Reuses
//!   [`HttpClub`](crate::club::HttpClub), so any OpenAI-compatible endpoint works.
//!
//! The approval policy is the safety boundary: for command placements, a denylist
//! (no `sudo`, `rm`, command-substitution, pipe-to-shell, exfil tools…) plus a
//! program allowlist, stricter for remote. Phone skips the command policy (it
//! sends text to a model, it doesn't execute) but is gated by `ANGEL_SWARM_PHONE`.
//! Off by default; enabled per run with `ANGEL_SWARM_DELEGATE` (folded into
//! `ANGEL_SWARM_MAX`); the non-local placements each need their own opt-in.

use crate::club::{Club, HttpClub};
use crate::sandbox::{self, SandboxPolicy};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// Where a delegator asked to run a test.
#[derive(Clone, Debug, PartialEq)]
pub enum Placement {
    Local,
    Remote(String), // fleet host label, e.g. "spark"
    Peer(String),   // network agent name/mailbox
    Phone(String),  // model key or category alias, e.g. "math" / "spark-r1"
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

    /// Exact coalescing scope for the approval gate. An approve-all for one
    /// host, peer, or model must never authorize a different destination.
    pub fn approval_scope(&self) -> crate::approval::ApprovalScope {
        match self {
            Placement::Local => crate::approval::ApprovalScope::SelfTest,
            Placement::Remote(host) => crate::approval::ApprovalScope::RemoteHost(host.clone()),
            Placement::Peer(peer) => crate::approval::ApprovalScope::Peer(peer.clone()),
            Placement::Phone(model) => crate::approval::ApprovalScope::PhoneModel(model.clone()),
        }
    }

    /// Local runs in the landlock sandbox (cheap, safe) → auto. Everything else
    /// reaches off the box (network / external API / another agent) → gate it.
    pub fn needs_approval(&self) -> bool {
        !matches!(self, Placement::Local)
    }
}

/// A test request parsed out of a delegator's draft.
#[derive(Clone, Debug)]
pub struct TestRequest {
    pub claim: String,
    pub cmd: String,
    pub placement: Placement,
    pub why: String,
}

/// The outcome of routing + running one request.
pub struct TestResult {
    pub request: TestRequest,
    pub verdict: &'static str, // pass | fail | timeout | denied | error | second-opinion | phoned
    pub detail: String,
}

// ---------------------------------------------------------------------------
// Parsing the `swarm-test` block out of a draft
// ---------------------------------------------------------------------------

const FENCE: &str = "```swarm-test";

/// Extract every `swarm-test` block from a draft. Tolerant of surrounding prose.
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

/// Remove the `swarm-test` blocks from a draft so the synthesis never echoes the
/// raw request (it gets the executed *evidence* instead).
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

/// Preserve first-seen order while collapsing identical requests from many
/// delegator drafts. Without this, a consensus claim can consume the entire
/// test budget by running the same command concurrently several times.
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
    // A phone consultation can use the claim itself as its question (the runner
    // already supports this). Executable placements still require a command.
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
    let s = s.split('#').next().unwrap_or(s).trim(); // drop trailing comments
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
        // A typo in an off-box placement must never silently become a local
        // shell execution. Drop the malformed request instead.
        None
    }
}

// ---------------------------------------------------------------------------
// The approver / router ("angel")
// ---------------------------------------------------------------------------

/// Programs allowed to *start* a local command (after path-stripping).
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

/// Tighter allowlist for remote hosts (no landlock out there).
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

/// Programs that are never allowed, matched as whitespace/operator-delimited tokens.
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

/// Substrings that are never allowed anywhere in a command.
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

/// The router/approver, configured from env.
pub struct Router {
    /// Scratch dir for local tests (writes confined here; default under $TMPDIR).
    workspace: PathBuf,
    remote_enabled: bool,
    peer_enabled: bool,
    phone_enabled: bool,
    /// Command run (via `sh -c`) to hand a request to a peer agent. Operator-set,
    /// never model-controlled; the model only fills `$SWARM_PEER_AGENT`/`_PROMPT`.
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
                // Best-guess default for robusty/mq9; override to match your CLI.
                "robusty-mq9-request send \"$SWARM_PEER_AGENT\" \"$SWARM_PEER_PROMPT\"".to_string()
            }),
            timeout: Duration::from_secs(secs.max(1)),
        }
    }

    /// Auto-approval: validate the request against policy.
    fn approve(&self, req: &TestRequest) -> Result<(), String> {
        // Phone is a model-API consult, not command execution — skip the command
        // policy; just require the flag and a resolvable target.
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

    /// Approve + run a single request, returning a verdict either way.
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
        // Unless explicitly disabled (ANGEL_SANDBOX=0), writes are confined to
        // the scratch dir + /tmp — NOT $HOME or the repo. Disarmed (the
        // default), the delegate runs unconfined like every write-mode agent.
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
        // The peer agent gets the claim + proposed test and renders a verdict; we
        // don't run the command ourselves. peer_cmd is operator-controlled.
        let prompt = format!(
            "Second opinion requested. Claim: {}\nProposed test: {}\nEvaluate the claim (run \
             the test if you can) and reply with a verdict and brief reasoning.",
            req.claim, req.cmd
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

    /// The "sota phone": consult a model API for a sub-problem / second opinion.
    fn run_phone(&self, req: &TestRequest, target: &str) -> TestResult {
        let Some((url, model, key)) = phone_target(target) else {
            return self.errored(req, format!("unknown phone target '{target}'"));
        };
        let question = if req.cmd.trim().is_empty() {
            req.claim.as_str()
        } else {
            req.cmd.as_str()
        };
        let context = if req.claim.trim().is_empty() {
            String::new()
        } else {
            format!("Context — the claim under consideration: {}\n\n", req.claim)
        };
        let prompt = format!(
            "You are being consulted by a peer agent for your expertise. Answer concisely and \
             decisively.\n\n{context}Question: {question}"
        );
        let club = HttpClub::new(format!("phone:{target}"), url, model, key);
        match club.respond(&prompt) {
            Ok(ans) if !ans.trim().is_empty() => TestResult {
                request: req.clone(),
                verdict: "phoned",
                detail: ans,
            },
            Ok(_) => self.errored(req, "phone returned an empty answer".to_string()),
            Err(e) => self.errored(req, format!("phone failed: {e}")),
        }
    }

    /// Spawn `cmd`, capture, and turn the exit status into a verdict.
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

/// Format executed results into an evidence block injected into the synthesis.
pub fn evidence_block(results: &[TestResult]) -> String {
    let mut s = String::from(
        "Executed test evidence — ground your answer in these real results. A delegator agent \
         proposed each; angel approved and routed it. Trust passing/failing tests and consulted \
         specialists over the drafts' bare assertions:\n",
    );
    for r in results {
        let claim = if r.request.claim.is_empty() {
            "(unstated)"
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Every executable in a shell chain/pipeline must be allowlisted. The old
/// first-token check let `cargo test; arbitrary-program` and `env arbitrary`
/// through because only `cargo`/`env` was inspected.
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
        // Keep the useful `env K=V command` form, but reject env flags: option
        // arity is platform-specific and ambiguity is an allowlist bypass.
        if words.get(i).is_some_and(|w| w.starts_with('-')) {
            return None;
        }
        while words.get(i).is_some_and(|w| is_assignment(w)) {
            i += 1;
        }
    }
    words.get(i).map(|_| i)
}

/// Remote execution has no landlock on the destination. `python -c` turns an
/// otherwise narrow executable allowlist into arbitrary code, so reject the
/// obvious inline-code modes there while retaining script/module test runs.
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

/// Minimal quote-aware shell lexer for command boundaries. It does not try to
/// execute or fully parse shell syntax; it only extracts each command's words
/// across `;`, `&&`, `||`, pipes, newlines, and grouping delimiters.
fn shell_segments(cmd: &str) -> Option<Vec<Vec<String>>> {
    // Command substitution is execution hidden inside an otherwise harmless
    // token (`echo `program`` / `echo $(program)`). The outer command alone is
    // not enough to validate it, so fail closed before lexing.
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
            // `2>&1` is a redirection token, not a background command boundary.
            '&' if word.ends_with('>') || word.ends_with('<') => word.push(c),
            '&' if chars.peek() == Some(&'&') => {
                chars.next();
                finish_segment(&mut word, &mut words, &mut segments);
            }
            // Background jobs can outlive the shell and turn a quick `pass` into
            // a false verdict. Delegated tests must remain foreground/bounded.
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

/// A disallowed pattern or program token in `cmd`, if any.
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

/// Map a fleet host label to an explicitly configured `user@host` SSH target.
/// Remote execution has no compiled username or endpoint fallback.
fn ssh_target(host: &str) -> Option<String> {
    let up = host.to_ascii_uppercase().replace('-', "_");
    std::env::var(format!("ANGEL_SWARM_SSH_{up}"))
        .ok()
        .map(|target| target.trim().to_string())
        .filter(|target| !target.is_empty())
}

/// Resolve a phone target (model key or category alias) → (base_url, model, key).
/// Category aliases map to a fleet model; any key is overridable via
/// `ANGEL_PHONE_<KEY>_URL/_MODEL/_KEY` (falling back to the Bag's `ANGEL_<KEY>_*`),
/// so external SOTA APIs are wired purely by env. Unknown + unconfigured → None.
fn phone_target(key: &str) -> Option<(String, String, Option<String>)> {
    // The default / first phone is DeepSeek — a strong external SOTA second
    // opinion, and a *different* model from gemma (which the swarm already runs
    // on, so phoning it would be phoning yourself). Category aliases map to the
    // model best at that "bit" of problem.
    let key = match key.trim() {
        "" | "default" | "sota" | "deep" | "ds" | "math" => "deepseek",
        "code" | "coder" => "spark",
        "reason" | "reasoning" => "atlas",
        "fast" | "quick" => "turbo",
        "flash" => "deepseek-flash",
        // Spark-local ds4 serve (DeepSeek-V4-Flash on the GB10) — free fleet seat.
        "dsflash" | "ds4" | "ds-flash" | "spark-flash" => "dsflash",
        "r1" | "deepseek-r1" => "spark-r1",
        k => k,
    };
    let up = key.to_ascii_uppercase().replace('-', "_");
    // (base_url, model, key-env-var). DeepSeek (external SOTA) is the default.
    // Fleet boxes carry NO baked-in model id — checkpoints churn on the rigs
    // daily, so an empty model lets HttpClub resolve whatever the endpoint's
    // live `/models` reports (an env pin still wins below).
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
        // Fleet aliases deliberately carry no endpoint. Configure their
        // ANGEL_PHONE_<KEY>_URL (or ANGEL_<KEY>_URL) route explicitly; the empty
        // model retains live `/models` resolution when no model pin is supplied.
        "spark" | "spark-r1" | "turbo" | "atlas" => Some((None, "", None)),
        // The Spark-local ds4 serve keeps its own served checkpoint id: a
        // text-only V4 serve, never the cloud V4.1 Flash. Route-scoped capability
        // keeps it text-only, and ANGEL_DSFLASH_MODEL still overrides the pin, so
        // no local model id disappears into an empty lookup.
        "dsflash" => Some((None, "deepseek-v4-flash", None)),
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

/// char-safe truncation.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…[truncated]")
    }
}

/// Run `cmd` with a kill-on-timeout deadline, draining pipes in threads so a full
/// pipe can't deadlock. (Mirrors the harness's `output_timed`.)
fn run_capture(cmd: Command, timeout: Duration) -> Result<(std::process::Output, bool), String> {
    // Use the harness's hardened runner: bounded 1 MiB pipe capture, exact
    // deadline wake-up, and process-group kill so a timed-out test cannot leave
    // grandchildren holding pipes (or GPUs) indefinitely.
    crate::harness::output_timed(cmd, Some(timeout))
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "../../tests/cockpit/app/swarm_delegate__tests.rs"]
mod tests;
