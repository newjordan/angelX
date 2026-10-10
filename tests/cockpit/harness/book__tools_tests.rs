//! Volume II, the tool library: every sentence moved off a tool schema — its
//! guidance and each parameter's description — is a verbatim page of that
//! tool's section; every schema names its section and every parameter is its
//! page address.
use super::super::{DIGITS, Route, Sub, TOC, ledger, primary};

/// Each tool's section and the sentences its schema carried, in order: the
/// guidance moved off its description, then each parameter's description. A
/// page dropped, merged or paraphrased fails here.
const ORIGINAL_TOOL_NOTES: &[(&str, &str, &[&str])] = &[
    (
        "⠡⠁",
        "read_file",
        &[
            "Use `agent://hnd_…` or handle_read for opaque handles; `outline://` always requires a workspace source path.",
            "Reuse the next offset named by a truncated page",
            "Each page is headed with `[path#tag]` (whole-file content tag) and absolute 1-based line numbers so you can author token-cheap hashline `apply_patch` edits that emit only the NEW lines.",
            "workspace path, or conflict:// / skill:// / agent:// / outline:// / pr:// / issue:// / ledger:// virtual URL",
            "one-based first source line; default 1.",
            "complete source lines to return; default 200, maximum 400",
        ],
    ),
    (
        "⠡⠃",
        "str_replace",
        &[
            "Fails if 'old' is absent or appears more than once — include enough surrounding context to make it unique.",
            "An indentation-only miss can return exact `old` and `expect_tag` recovery fields; copy them and author `new` with the intended indentation.",
            "path inside the workspace (relative or absolute)",
            "exact text to replace (must be unique)",
            "replacement text",
            "optional stale-edit guard: the file's content tag from read_file's [path#tag] header.",
            "The edit is rejected if the live file no longer matches it.",
        ],
    ),
    (
        "⠡⠉",
        "git_diff",
        &[
            "Review your own edits before committing.",
            "diff the index (--cached) instead of the worktree",
            "compare the current worktree against one verified branch, tag, or commit",
            "show a compact file/change summary instead of the full patch",
            "limit the diff to this in-workspace path (relative or absolute)",
        ],
    ),
    (
        "⠡⠙",
        "git_log",
        &[
            "Orient on what changed recently and why.",
            "how many commits (default 15, max 100)",
            "limit history to this in-workspace path (relative or absolute)",
        ],
    ),
    (
        "⠡⠑",
        "file_search",
        &[
            "Use when you half-remember a filename; complements find_files (exact glob) and grep (by content).",
            "include ignored/generated paths (default false); credential and quarantine exclusions still apply",
            "include hidden paths (default false); .git and credential files remain excluded",
            "path fragment to fuzzy-match",
            "max results (default 20)",
        ],
    ),
    (
        "⠡⠋",
        "semantic_read",
        &[
            "This is useful after file_search/grep finds plausible files but reading all of them would waste model context.",
            "natural-language question or implementation concept to retrieve",
            "workspace files selected by file_search, grep, find_files, or prior evidence",
            "ranked passages to return (default 6, maximum 12)",
        ],
    ),
    (
        "⠡⠛",
        "lsp_diagnostics",
        &[
            "Use to verify an edit compiles/type-checks instead of guessing.",
            "File to analyze (workspace-relative or absolute).",
        ],
    ),
    (
        "⠡⠓",
        "notes",
        &[
            "Use for durable findings and decisions.",
            "default 'list'",
            "for action=add",
        ],
    ),
    (
        "⠡⠊",
        "handoff",
        &[
            "Write the state a cold successor needs to resume this work: current goal, what is done (with evidence), what is in flight, exact next steps in order, and load-bearing facts (paths, hashes, commands, decisions with reasons).",
            "Refresh it before long or risky stretches and before ending a run.",
            "default: 'write' when 'note' is present, else 'show'",
            "for action=write — the complete replacement resume brief",
        ],
    ),
    (
        "⠡⠚",
        "recall",
        &[
            "Use it when prior context would help and isn't in the current conversation.",
            "What to recall.",
            "Max notes (default 5).",
        ],
    ),
    (
        "⠩⠁",
        "shell",
        &[
            "Act on actual tool evidence: preserve the earliest prerequisite failure, check usable input before dependent measurements, and discover optional dependencies from real errors.",
            "Do not score failed input as zero performance.",
            "Never use shell `sleep` to poll a job or submission; keep doing useful work and use one later status snapshot.",
            "Omit scope options for builds, benchmarks, installs and background jobs.",
            "Act on actual errors: keep the earliest prerequisite failure, check usable input before dependent measurements, use allowed scratch, and do not score failed input as zero performance.",
            "Explicit restrictions stay authoritative; report a concrete blocker if no permitted approach works.",
            "Judge from actual exit status and output; quoted text, a quiet success, and a valid `$url` are not empty-input or zero-frame evidence.",
            "This sealed task requires foreground process ownership: run long builds directly; nohup/disown/setsid and unmanaged `&` jobs are rejected.",
            "This sealed task permits read-only Git inspection only; edit files directly and do not stash/add/reset/restore/checkout/clean/commit/switch/fetch/merge/rebase/push.",
            "YOLO is active for this headless coding task: writes/network are unrestricted, but default cwd is the task workspace — start with local `ls`/`find .`/`tests` there.",
        ],
    ),
    (
        "⠩⠃",
        "cargo",
        &[
            "Use this to pull crates and build/calibrate code.",
            "Pass subcommand + flags as `args`, e.g. build or test.",
            "Use available local dependencies; network fetches are unavailable in this confined experiment.",
            "cargo subcommand + flags",
        ],
    ),
    (
        "⠩⠉",
        "lint",
        &[
            "Select runtime explicitly for mixed roots.",
            "Runtime selection.",
            "Auto requires one root language; mixed projects need explicit selection.",
            "Extra cargo clippy arguments on the Rust route; other lint adapters are unsupported.",
        ],
    ),
    (
        "⠩⠙",
        "proc_run",
        &[
            "Do not redirect output: proc_status reads the captured log.",
            "Use for daemons and slow jobs — an inference server (llama-server, vllm serve), a dev server, a long build — then keep working.",
            "Take one proc_status snapshot only when its result can change your next action; if no independent work remains, use proc_wait for a bounded wait that returns early on completion rather than shell sleep.",
            "Use `shell` for commands that finish quickly.",
            "command line (sh -c)",
            "short label for the handle/log (default: first word)",
            "working directory (default: the workspace)",
        ],
    ),
    (
        "⠩⠑",
        "proc_status",
        &[
            "If still running, do useful independent work.",
            "If all remaining work depends on this job, use proc_wait instead of shell sleep.",
            "Use contains with an id to search captured compiler errors by literal substring; captured host logs are not workspace files for grep/read_file.",
            "handle id from proc_run",
            "log lines to show for a single id (default 40)",
            "With id only: literal case-sensitive log filter, 1–256 UTF-8 bytes, no newlines.",
            "Searches trailing 8 MiB of each retained log file; tail_lines limits matching lines.",
            "ignored (legacy).",
            "Snapshot only; never parks the turn.",
        ],
    ),
    (
        "⠩⠋",
        "proc_wait",
        &[
            "Wait for one background job when no useful independent work remains.",
            "Use instead of shell sleep; use proc_status for an immediate snapshot.",
            "handle id from proc_run",
            "captured log lines (default 40, maximum 400)",
        ],
    ),
    (
        "⠩⠛",
        "tool_repair",
        &[
            "Call only when the user has explicitly asked to investigate or fix a broken tool in this session.",
            "Diagnose-only by default; pass repair=true only after the user approves making changes.",
            "Set true ONLY after the user approves making changes.",
            "command/profile to repair, e.g. auto or gh (default auto)",
            "apply local repairs (mutates ~/.local/bin symlinks and global git config); default false — diagnose only.",
        ],
    ),
    (
        "⠩⠓",
        "loop_research",
        &[
            "Continue ordinary work, inspect status/results, stop when desired, and apply/verify useful patches yourself.",
            "suggest: optional unmeasured ideas; no fabricated rewards",
            "Defaults to loop objective; selects a separate learning scope if changed",
            "Defaults to loop verifier; explicit null explores without learning rewards",
            "suggest: choose any methods; defaults to all",
            "suggest: deterministic exploration seed; default 0",
            "run: the experimental approach to try",
            "run: stable label for bandit comparisons; default direct",
            "run: opt into baseline plus candidate (two attempts); default false",
            "run: include retrieved experimental snippets as task context; default true",
        ],
    ),
    (
        "⠩⠊",
        "rl_campaign",
        &[
            "Continue useful work while it runs; results retains artifacts across iterations and resumes.",
            "Use real checks, inspect candidates, and submit a verified winner when ready.",
            "Optional research objective; defaults to loop task",
            "Actual verifier command; defaults to loop acceptance command",
            "Campaign rounds; default 1",
            "Samples per training group; default 3",
            "Samples per measured comparison case; at least 2, default 2",
            "Existing verifier-owned paths relative to each case source",
            "Existing independently authored held-out source workspace",
            "results: optional exact campaign id, within this workspace",
        ],
    ),
    (
        "⠩⠚",
        "benchmark_compare",
        &[
            "Samples must be nonnegative and matched in order; separate diagnostic, full-development and official cohorts.",
            "Metric and unit, e.g. kernel latency (ms).",
        ],
    ),
    (
        "⠹⠁",
        "delegate",
        &[
            "Use mode=review/read_only for inspection-only reviewers; use mode=write for implementation, then call `integrate` with that branch to apply it.",
            "specialist club name",
            "what the specialist should do",
            "write implements changes; review/read_only inspects with workspace writes blocked",
        ],
    ),
    (
        "⠹⠃",
        "swarm_compile",
        &[
            "Use direct action instead when a trustworthy red verifier already exists.",
            "required for resume/status",
            "coding outcome to produce",
            "optional stable class used by learned routing",
            "must fail on the immutable test-only branch with the supplied run marker and pass on the candidate",
            "full acceptance command; must pass on base and candidate",
            "optional candidate-only lint/build/typecheck commands (max 4)",
            "allowed test-file paths/prefixes; implementation may not edit files the test author changes",
            "auto, self, or one explicit route for all roles",
            "optional per-role route overrides for investigator/test_author/implementer/reviewer",
        ],
    ),
    (
        "⠹⠉",
        "consult_model",
        &[
            "method=deli optionally runs fresh-context Deli deliberation and returns its findings to this turn; choose rounds when useful, then continue ordinary work.",
            "Deli reasons over the supplied material; use normal tools or RL campaigns to measure its proposals.",
            "Prefer local seats; paid SOTA needs operator allow.",
            "club or model name (default auto=self).",
            "the task, code snippet, or question to consult on",
            "optional role instructions for the consulted model",
            "direct (default): one consultation; deli: iterative deliberation, then return to this task",
            "Deli rounds for this call; defaults to the operator's ANGEL_DELI_ROUNDS (6)",
        ],
    ),
    (
        "⠹⠙",
        "agent_graph",
        &[
            "Use for work that genuinely splits into specialties with handoffs; for ad-hoc parallel seats use `spawn`, and for most tasks just do the work yourself.",
            "graph name from the installed catalog",
            "the task the graph works",
        ],
    ),
    (
        "⠹⠑",
        "spawn",
        &[
            "For direct single-call model questions use `consult_model` or `code_review`; for git-isolated implementation use `delegate`.",
            "seat tool grant; code requires n=1 (use delegate for parallel writes)",
            "the task every seat works (or use tasks[])",
            "one task per seat (sets n)",
            "default: panel when n>1, else solo",
            "seat count (default 1 for solo, else 3; capped by ANGEL_SPAWN_MAX)",
            "optional exact persona name or list cycling across seats; installed names: {}; omit this field for a plain seat",
            "self/auto = your own model replicated (default, self-same panel) | smart = the designated escalation seat (ANGEL_SOTA_SMART_CLUB, default luna) | fleet = spread across reachable fleet clubs (opt-in) | an explicit club label",
            "optional per-seat clock in seconds, started at the seat's first response (the wait for that response gets its own equal window); default 900 (operator ANGEL_SPAWN_TIMEOUT); a cut seat returns its partial work",
            "K for quorum formation (default ceil(n/2))",
        ],
    ),
    (
        "⠹⠋",
        "code_mode",
        &[
            "Handle intermediates with `handle_put(body, {producer?, identity?})` → opaque `hnd_…` and `handle_get(id, {offset?, max_bytes?})` for a capped slice — keep bulk out of the script return value.",
            "Use handle_put/handle_get for bulk.",
            "`return` your final value (objects are JSON-stringified).",
            "Use `recipe:'repo_recon', query:'the task'` for the fixed, read-only task-conditioned repository map, or fan out custom inspection.",
            "Example: `const r = batch(files.map(f => ({tool:'outline', args:{path:f}}))); return r.filter(x => !x.ok).length + ' failing inspections';`",
            "`return` the result.",
            "JavaScript to execute.",
            "Call bound tools as functions; `return` the final value.",
            "Trusted built-in orchestration recipe.",
            "Mutually exclusive with `script`.",
        ],
    ),
    (
        "⠹⠛",
        "handle_read",
        &[
            "Prefer strategy over bulk: only call when the receipt is insufficient.",
            "Opaque handle id from a prior receipt (hnd_…).",
            "Byte offset into the stored body.",
            "Requested slice size; further capped by store policy.",
        ],
    ),
    (
        "⠹⠓",
        "tool_search",
        &[
            "Use this when you need a capability not in your base tool list.",
            "Returns matching tool names + summaries; then call the tool directly by name.",
            "capability keywords",
            "max results (default 5)",
        ],
    ),
    (
        "⠹⠊",
        "skill",
        &[
            "Load a skill's full instructions by name before doing that kind of task.",
            "skill name from the catalog",
        ],
    ),
    (
        "⠹⠚",
        "jev_decide",
        &[
            "Batch independent questions about the same state.",
            "Never use as a verifier or as measured benchmark improvement; use benchmark_compare for measured percentages.",
            "Sends only supplied state/questions to TypeSafe; omit secrets.",
            "Include dataset, source and uncertainty.",
            "No credentials.",
            "Concise evidence, code excerpts or measurement records.",
            "Explicit question; the model does not see the question id.",
            "For choice: distinct candidate descriptions.",
            "For score: ordered rubric levels from 0 upwards.",
            "Omit for noul.",
        ],
    ),
    (
        "⠱⠁",
        "web_search",
        &[
            "Use for current information, documentation, error messages, or anything outside the workspace.",
            "Research search origin: {origin}; fetch document URLs returned by web_search with web_fetch.",
            "Do not probe the host with shell.",
            "Research search origin: {origin}; fetch document URLs with web_fetch.",
            "search query",
            "max results (default 5)",
        ],
    ),
    (
        "⠱⠃",
        "web_fetch",
        &[
            "Use to read documentation, APIs, or pages found via web_search.",
            "Research search origin: {origin}; fetch document URLs returned by web_search with web_fetch.",
            "Do not probe the host with shell.",
            "Research search origin: {origin}; fetch document URLs with web_fetch.",
            "http(s) URL to fetch",
            "cap on returned text bytes (default 20000, max 200000)",
        ],
    ),
    (
        "⠱⠉",
        "science_search",
        &[
            "Use for papers, prior work, citation counts, or a literature review in ML, biology, physics or chemistry.",
            "the research question or topic",
            "papers per source (default 8, max 25)",
        ],
    ),
    (
        "⠱⠙",
        "repo_search",
        &[
            "Use when a report or answer should point at real, maintained projects: reference implementations, libraries, tools, or the freshest work in a fast-moving area.",
            "topic or GitHub search query (supports GitHub qualifiers like language:rust, stars:>1000)",
            "best = most-starred (default); latest = most-recently-pushed",
            "repositories to return (default 10, max 25)",
        ],
    ),
    (
        "⠱⠑",
        "grok_research",
        &[
            "Prefer over web_search when recency, breaking news, or social/X signal matters — it runs Grok's own agentic web + X search and brings back cited context.",
            "what to research (a question or topic)",
        ],
    ),
    (
        "⠱⠋",
        "http_request",
        &[
            "Use for JSON APIs — local inference servers, webhooks, REST services; use web_fetch for reading pages.",
            "HTTP method (default GET)",
            "http(s) URL",
            "header name → value",
            "request body (verbatim)",
            "cap on returned body bytes (default 20000, max 200000)",
        ],
    ),
    (
        "⠱⠛",
        "fleet_status",
        &["Use to find which rigs are reachable before SSHing or probing an endpoint on one."],
    ),
    (
        "⠱⠓",
        "vast_instances",
        &[
            "Recon only — report an idle instance, never destroy or restart one; that is the operator's call.",
        ],
    ),
    (
        "⠱⠊",
        "llm_probe",
        &[
            "Use after starting a server with proc_run to confirm it's ready, or to discover what a fleet box serves.",
            "endpoint base, e.g. http://host:8000 (with or without /v1)",
            "authentication credential if required",
        ],
    ),
    (
        "⠱⠚",
        "llm_bench",
        &[
            "Meant for local/self-hosted endpoints — pointing it at a paid API spends real tokens.",
            "endpoint base, e.g. http://host:8000 (with or without /v1)",
            "model id (default: first id from /v1/models)",
            "fixed prompt (default: a ~40-token instruction)",
            "per run (default 128)",
            "measured runs after warmup (default 3, max 10)",
            "authentication credential if required",
        ],
    ),
    (
        "⠫⠁",
        "present",
        &[
            "Use this when asked to show work, images, videos, or reports: image/video displays a local artifact; resource/report displays a local UTF-8 document (including Markdown, source, CSV, or JSON).",
            "Keep the artifact's actual path and a descriptive label; do not substitute example work.",
            "image/video = local terminal-native preview; resource/report = local UTF-8 document; link/graph = exact URL or local report",
            "Short label shown on the card.",
            "Actual local artifact path (relative to the active workspace or absolute), or an exact http(s) URL for a link.",
        ],
    ),
    (
        "⠫⠃",
        "video_probe",
        &[
            "Use before planning an edit so clip math is grounded in real container facts.",
            "media file, workspace-relative",
        ],
    ),
    (
        "⠫⠉",
        "video_beats",
        &[
            "Cut points should snap to these beats — music owns time.",
            "Analyze a music track and return its beat grid as JSON: estimated BPM, beat timestamps, onset (transient) timestamps, and windowed RMS energy so you can hear where phrases and the crescendo live.",
            "audio file, workspace-relative",
            "seconds per energy window (default 5)",
        ],
    ),
    (
        "⠫⠙",
        "video_cut",
        &[
            "Beat-snap the boundaries yourself using video_beats output — this tool renders exactly what you specify.",
            "Give clips as {path, in, out} (seconds) in timeline order; segment boundaries are joined with a crossfade of `fade` seconds (0 = hard cuts).",
            "source in-point, seconds",
            "source out-point, seconds",
            "destination mp4, workspace-relative",
            "optional audio bed, workspace-relative",
            "seconds into the music to start (default 0)",
            "crossfade seconds between clips (default 0 = hard cuts)",
            "audio fade-out length at the end (default 1.0)",
            "output width (default: first clip's)",
        ],
    ),
    (
        "⠫⠑",
        "video_contact_sheet",
        &[
            "Feed the sheet to video_look for a machine read, or present() it to the director for circle/selects decisions.",
            "video file, workspace-relative",
            "destination png/jpg, workspace-relative",
            "number of samples (default 12, max 64)",
            "grid columns (default 4)",
        ],
    ),
    (
        "⠫⠋",
        "video_look",
        &[
            "Use after video_contact_sheet for single-pass take review, or directly with `frames`/`timestamps` for targeted questions.",
            "video file (extract frames) or image file (sent as-is), workspace-relative",
            "what to look for, e.g. 'rank the takes on handoff cleanliness' or 'describe palette and framing of each shot'",
            "evenly spaced frames to extract when path is a video (default 6, max 8; ignored for images)",
            "exact timestamps (seconds) to grab instead of even spacing; overrides frames",
        ],
    ),
    (
        "⠫⠛",
        "vision_look",
        &[
            "Prefer this over inventing visual details.",
            "image or video path, workspace-relative",
            "what to look for, e.g. 'OCR all text' or 'describe the UI state'",
            "evenly spaced frames when path is a video (default 4, max 8; ignored for images)",
            "exact timestamps (seconds) for video frames; overrides frames",
        ],
    ),
    (
        "⠫⠓",
        "knowledge_graph",
        &[
            "Facts persist across sessions — use ingest for documents worth remembering, query before re-reading sources.",
            "pipeline stage to run",
            "ingest: the document text",
            "ingest: short source id for provenance (e.g. a path or URL)",
            "query: the question to answer from the graph",
            "optional club label; default self (the in-hand driver)",
        ],
    ),
    (
        "⠫⠊",
        "continual_harness",
        &[
            "Prefer small evidence-backed edits after a repeated failure, reusable tactic, or durable preference.",
            "default list",
            "default project",
            "required for create/update",
            "entry id (or refinement id for rollback)",
            "grouping path, default general",
            "why this edit is justified",
            "expected improvement",
        ],
    ),
    (
        "⠫⠚",
        "work_landing",
        &[
            "Establish your WORK CONTEXT for this workspace at the start of a conversation.",
            "Then ASK the user to confirm.",
            "Call again with confirm=true (plus repo / visibility / mode overrides if they corrected anything) to RECORD the confirmed context, which is persisted and drives your behavior mode for this workspace.",
            "record the user's confirmation (sets the context as established)",
            "owner/repo override when confirming (if detection missed/misread it)",
            "visibility override when confirming",
            "behavior-mode override when confirming (else inferred from visibility)",
        ],
    ),
    (
        "⠣⠁",
        "outline",
        &["path inside the workspace (relative or absolute)"],
    ),
    (
        "⠣⠃",
        "list_dir",
        &[
            "filename fragment to rank first (exact, substring, then fuzzy); does not filter",
            "basename glob to rank first; does not filter",
            "zero-based entry offset in the sorted listing",
            "page size (default 700); 24000-byte page window also applies; omitted counts and continuation are reported",
            "include ignored/generated paths (default false); credential and quarantine exclusions still apply",
            "include hidden paths (default false); .git and credential files remain excluded",
            "workspace-relative directory (default '.')",
        ],
    ),
    (
        "⠣⠉",
        "grep",
        &[
            "include ignored/generated paths (default false); credential and quarantine exclusions still apply",
            "include hidden paths (default false); .git and credential files remain excluded",
            "regular expression",
            "file or directory inside the workspace, relative or absolute (default '.'); may be combined with `paths` — both are merged and deduplicated",
            "1-8 files/directories to search as one sorted, deduplicated union; both `path` and `paths` together are accepted (merged, deduplicated)",
            "deterministic continuation cursor from a prior grep receipt; only files lexically after this workspace-relative path are considered",
            "workspace-relative files to omit (maximum 128); useful for bounded explicit resumption/exclusion",
            "case-insensitive (default false)",
            "lines before and after each match (default 0, maximum 10); overlapping windows are merged",
        ],
    ),
    (
        "⠣⠙",
        "find_files",
        &[
            "include ignored/generated paths (default false); credential and quarantine exclusions still apply",
            "include hidden paths (default false); .git and credential files remain excluded",
            "glob pattern, matched against workspace-relative paths",
        ],
    ),
    (
        "⠣⠑",
        "defs",
        &[
            "Set include_source with 1-8 discovered file paths to collect bounded definition and reference windows with file hashes in one read per file.",
            "Provide exactly one of name/names.",
            "optional workspace file or directory to scope; outside targets are refused",
            "one symbol name to locate",
            "1-8 symbol names to resolve in one workspace scan",
            "case-insensitive symbol matching (default false)",
            "return JSON source windows from explicit paths (default false)",
            "discovered source files for include_source; no directories",
            "total returned source text bytes, excluding JSON metadata (default 16384)",
        ],
    ),
    (
        "⠣⠋",
        "git_status",
        &[
            "A compact orientation before editing or committing; complements git_diff (which shows the actual changes).",
        ],
    ),
    (
        "⠣⠛",
        "self_map",
        &[
            "Use this to understand or plan changes to yourself.",
            "source module stem, e.g. agent/harness/registry (the layer prefix is optional)",
            "a module stem (e.g. \"harness\", \"swarm\", \"tools/nav\") for its outline only",
            "also persist the full map to SELF.md at the crate root",
        ],
    ),
    (
        "⠷⠁",
        "write_file",
        &[
            "workspace path, or conflict://N / conflict://* virtual URL",
            "full file contents, or @ours/@theirs/@base/@both for conflict resolve",
        ],
    ),
    (
        "⠷⠃",
        "multi_edit",
        &[
            "Fewer hops than repeated str_replace for a multi-site refactor.",
            "path inside the workspace (relative or absolute)",
            "optional stale-edit guard: the file's content tag from read_file's [path#tag] header.",
            "All edits are rejected if the live file no longer matches it.",
            "ordered edits, each applied to the result of the previous",
        ],
    ),
    (
        "⠷⠉",
        "apply_patch",
        &[
            "Hashline also supports stage=true: preflight and queue the plan without writing; call resolve_edit to accept or reject.",
            "unified diff (a/ b/ headers)",
            "path strip level -pN (default 1)",
            "hashline only: when true, preflight and stage the plan (no disk write); resolve with resolve_edit",
        ],
    ),
    (
        "⠷⠙",
        "resolve_edit",
        &[
            "accept | reject | list (default list)",
            "staged edit id from the proposal card (required for accept/reject)",
            "optional note recorded on accept/reject",
        ],
    ),
    (
        "⠷⠑",
        "git_commit",
        &[
            "Set execute=true to stage each unit and commit in that order.",
            "false (default) = print plan only; true = stage+commit each unit",
            "true (default) = one commit per file class; false = single commit",
            "commit subject (and optional blank-line + body).",
            "Used for the first/only unit; further split units get class-scoped subjects",
            "include untracked files in the plan (default true)",
        ],
    ),
    ("⠷⠋", "integrate", &["branch from delegate"]),
    (
        "⠾⠁",
        "run_tests",
        &[
            "typed runtime selection; an explicit entrypoint selects its runtime, otherwise auto requires one root language (optional)",
            "Explicit native test entrypoint; node bypasses package scripts, unittest accepts discover/modules/files, pytest requires an immutable system installation.",
            "alias of runtime for the scan fallback: rust|js|python|go|swift (optional)",
            "Rust: cargo test flags.",
            "Node: confined test files/globs and test-name/skip-pattern.",
            "Python: unittest -s directory, -p pattern, -k.",
            "Other runners: appended verbatim (optional)",
            "run the suite of this subdirectory of the workspace (mono-repos: e.g. `sidecar/forge` for its python tests, `cockpit` for that crate); the runner is chosen from that directory's own files (optional)",
            "alias of `dir` for repos whose crates live one directory down (e.g. cockpit/, harness/); default = the largest (optional)",
        ],
    ),
    (
        "⠾⠃",
        "check",
        &[
            "Runtime selection.",
            "Auto requires one root language; mixed projects need explicit selection.",
            "Rust: cargo check flags.",
            "Node/Python: confined source files or simple filename globs; default scans up to 256 visible source files, excluding generated/vendor directories.",
        ],
    ),
    ("⠾⠉", "fmt", &["check only, don't modify (default false)"]),
    ("⠾⠙", "proc_stop", &["handle id from proc_run"]),
    (
        "⠾⠑",
        "shell, continued",
        &[
            "Do not inventory `/`, `$HOME`, or unrelated repos; prefer `read_file`/`grep` inside the workspace.",
            "YOLO SMART is active: workspace shell/write batches do not wait for interactive approval — act decisively with tools (edit, build, test, fix).",
            "Prefer concrete tool-backed code changes over status prose.",
            "shell command.",
        ],
    ),
    (
        "⠾⠋",
        "loop_research, continued",
        &[
            "results: exact retained run within this workspace",
            "results: retained run count; default 5",
            "campaign: start, run, status, cancel, check, recover or recheck the native Labyrinth workflow; its role policies are registered at ⡬⠊.",
            "campaign: durable Labyrinth campaign identifier.",
            "campaign: complete specification with explicit doors, source files, independent checks and shared compute limits.",
            "campaign check: workspace-relative path to a complete frozen integration bundle; validate exact source and check receipts without a model call, canonical publication or another recheck.",
        ],
    ),
    (
        "⠾⠛",
        "rl_campaign, continued",
        &["results: number of retained campaigns; default 5"],
    ),
    (
        "⠯⠁",
        "todo",
        &[
            "default 'list'",
            "for action=add",
            "for action=complete",
            "for action=set",
        ],
    ),
    (
        "⠯⠃",
        "goal",
        &[
            "This does NOT start an autonomous loop — that is the operator's call.",
            "default 'show'",
            "the objective (action=set)",
            "optional verifiable command whose success means the goal is met (action=set)",
            "optional acceptance criteria (action=set)",
        ],
    ),
    (
        "⠯⠉",
        "get_context_remaining",
        &[
            "Report how many tokens the conversation holds against its compaction threshold; older turns compact automatically there, so the threshold never ends the work.",
        ],
    ),
    (
        "⠯⠙",
        "code_review",
        &[
            "file path, git diff, or raw code snippet to review",
            "review focus area",
            "optional club (default: self / in-hand)",
        ],
    ),
    ("⠯⠑", "reverse", &["input text"]),
    ("⠯⠋", "word_count", &["input text"]),
    (
        "⠯⠛",
        "code_mode, continued",
        &[
            "Task text used to condition `repo_recon`; encoded as data, never executable code.",
            "Request nested mutation or execution tools; also requires operator ANGEL_CODE_MODE_EFFECTS=1.",
        ],
    ),
    (
        "⠮⠁",
        "lsp_definition",
        &[
            "Give `symbol` (first occurrence is queried) or an explicit 1-based `line`(+`character`).",
            "Source file (workspace-relative or absolute).",
            "Symbol to locate (first occurrence in the file).",
            "Use this OR line/character.",
            "1-based line of the symbol (alternative to `symbol`).",
            "1-based column on `line` (default 1).",
        ],
    ),
    (
        "⠮⠃",
        "lsp_references",
        &[
            "Give `symbol` or 1-based `line`(+`character`).",
            "Source file (workspace-relative or absolute).",
            "Symbol to locate (first occurrence in the file).",
            "Use this OR line/character.",
            "1-based line of the symbol (alternative to `symbol`).",
            "1-based column on `line` (default 1).",
        ],
    ),
    (
        "⠮⠉",
        "lsp_hover",
        &[
            "Give `symbol` or 1-based `line`(+`character`).",
            "Source file (workspace-relative or absolute).",
            "Symbol to locate (first occurrence in the file).",
            "Use this OR line/character.",
            "1-based line of the symbol (alternative to `symbol`).",
            "1-based column on `line` (default 1).",
        ],
    ),
    (
        "⠮⠙",
        "lsp_symbols",
        &["Source file to outline (workspace-relative or absolute)."],
    ),
    (
        "⠮⠑",
        "lsp_workspace_symbol",
        &[
            "A server must be warm first (run lsp_symbols/lsp_diagnostics on any project file).",
            "Symbol name (or prefix/substring) to search for.",
        ],
    ),
    (
        "⠮⠋",
        "ui_inspect",
        &[
            "Use format=cells for exact Ratatui symbols, color variants, underline color, modifiers, and skip flags.",
            "Page an immutable cached snapshot without drawing a new frame.",
        ],
    ),
    (
        "⠮⠛",
        "ui_verify",
        &[
            "Page the immutable result later with ui_inspect(snapshot_id=...).",
            "Optional exact campaign id from the loaded report catalog.",
        ],
    ),
    (
        "⠮⠓",
        "video_cut, continued",
        &[
            "output height (default: first clip's)",
            "output frame rate (default 24)",
        ],
    ),
];

/// Volume II's chapters, in table-of-contents order.
const VOLUME_TWO: &str = "⠡⠩⠹⠱⠫⠣⠷⠾⠯⠮";

/// A section that carries a tool's pages past ten is named for it, continued.
const CONTINUED: &str = ", continued";

fn section(cells: &str) -> &'static Sub {
    let mut chars = cells.chars();
    let (cell, digit) = (chars.next().unwrap(), chars.next().unwrap());
    let route = Route::new(cell, digit);
    primary(cell)
        .and_then(|primary| primary.subs.iter().find(|sub| sub.route == route))
        .unwrap_or_else(|| panic!("{cells} is not a section"))
}

/// The tool a Volume II section belongs to.
fn tool_of(sub: &Sub) -> &'static str {
    sub.name.strip_suffix(CONTINUED).unwrap_or(sub.name)
}

/// A tool's sections: its own first, then its continuation.
fn sections_of(tool: &str) -> Vec<Route> {
    let mut subs: Vec<&Sub> = TOC
        .iter()
        .filter(|primary| {
            VOLUME_TWO.contains(primary.cell)
                || primary.cell == super::super::labyrinth_campaign::CELL
                || primary.cell == super::super::labyrinth_campaign::SHELF.cell
        })
        .flat_map(|primary| primary.subs.iter())
        .filter(|sub| tool_of(sub) == tool)
        .collect();
    subs.sort_by_key(|sub| sub.name.ends_with(CONTINUED));
    subs.into_iter().map(|sub| sub.route).collect()
}

#[test]
fn every_moved_sentence_is_a_verbatim_page_of_its_tools_section() {
    for (cells, tool, sentences) in ORIGINAL_TOOL_NOTES {
        let sub = section(cells);
        assert_eq!(sub.name, *tool, "{cells} is named for its tool");
        assert_eq!(sub.pages, *sentences, "{cells} lost or changed a sentence");
    }
}

#[test]
fn volume_two_is_one_section_per_tool_and_each_routes_somewhere() {
    let volume_two: Vec<_> = TOC
        .iter()
        .filter(|primary| VOLUME_TWO.contains(primary.cell))
        .collect();
    assert_eq!(
        volume_two.iter().map(|p| p.cell).collect::<String>(),
        VOLUME_TWO,
        "Volume II is ⠡ ⠩ ⠹ ⠱ ⠫ ⠣ ⠷ ⠾ ⠯ ⠮, in order"
    );
    let mut tools = std::collections::HashSet::new();
    for primary in volume_two {
        for (index, sub) in primary.subs.iter().enumerate() {
            assert_eq!(sub.route.sub, DIGITS[index], "{} out of order", sub.name);
            assert!(!sub.pages.is_empty(), "{} routes nowhere", sub.name);
            assert!(tools.insert(sub.name), "{} has two sections", sub.name);
            let decoded = ledger::read(std::path::Path::new("."), &sub.route.cells()).unwrap();
            assert!(
                decoded.contains(sub.pages[0]),
                "{} decodes its pages",
                sub.name
            );
        }
    }
    // A continuation follows a full section of its own tool.
    for name in &tools {
        if let Some(tool) = name.strip_suffix(CONTINUED) {
            let own = sections_of(tool);
            assert_eq!(own.len(), 2, "{tool} continues once");
            assert_eq!(own[0].sub().pages.len(), DIGITS.len(), "{tool} is full");
        }
    }
    assert_eq!(tools.len(), ORIGINAL_TOOL_NOTES.len());
}

fn is_braille(word: &str) -> bool {
    !word.is_empty() && word.chars().all(ledger::is_cell)
}

/// Does a page keep a `{placeholder}` for data after its address to fill?
fn has_placeholder(page: &str) -> bool {
    page.match_indices('{').any(|(at, _)| {
        let rest = &page[at + 1..];
        rest.find('}').is_some_and(|end| {
            rest[..end]
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_')
        })
    })
}

/// One word of page addresses, each into this tool's own sections; the last
/// page it names.
fn own_pages(tool: &str, word: &str, own: &[Route]) -> &'static str {
    let addresses =
        ledger::addresses(word).unwrap_or_else(|| panic!("{tool}: `{word}` is not addresses"));
    let mut last = "";
    for address in &addresses {
        let (Some(section), Some(page)) = (address.section, address.page) else {
            panic!("{tool}: `{word}` must address pages");
        };
        let route = Route::new(address.primary, section);
        assert!(
            own.contains(&route),
            "{tool}: `{word}` reaches another tool's section"
        );
        let index = DIGITS.iter().position(|digit| *digit == page).unwrap();
        last = route
            .sub()
            .pages
            .get(index)
            .unwrap_or_else(|| panic!("{tool}: `{word}` names no page"));
    }
    last
}

/// Page addresses and their data: data rides only right after an address
/// whose page keeps a placeholder for it.
fn check_pages_and_data(tool: &str, words: &[&str], own: &[Route]) {
    let mut fills = false;
    for word in words {
        if is_braille(word) {
            fills = has_placeholder(own_pages(tool, word, own));
        } else {
            assert!(fills, "{tool}: English `{word}` rides after no placeholder");
        }
    }
}

/// A description: what the tool does, then its route (its own section, and
/// its continuation when its guidance runs there), then only pages of its own
/// sections with the data their placeholders take. The routes it names, or
/// none when the tool has no section.
fn description_routes(def: &crate::agent::club::ToolDef, own: &[Route]) -> Vec<Route> {
    let words: Vec<&str> = def.description.split_whitespace().collect();
    let Some(at) = words.iter().position(|word| is_braille(word)) else {
        return Vec::new();
    };
    let routes: Vec<Route> = ledger::addresses(words[at])
        .unwrap_or_else(|| panic!("{}: `{}` is not a route", def.name, words[at]))
        .into_iter()
        .map(|address| {
            assert!(
                address.page.is_none(),
                "{}: the route names sections",
                def.name
            );
            Route::new(address.primary, address.section.expect("a section"))
        })
        .collect();
    assert!(routes.len() <= super::super::WARPATH_ROUTES);
    assert_eq!(
        routes.first(),
        own.first(),
        "{} names its own section first",
        def.name
    );
    assert!(
        routes.iter().all(|route| own.contains(route)),
        "{} names another tool's section",
        def.name
    );
    check_pages_and_data(&def.name, &words[at + 1..], own);
    routes
}

/// Every `description` in a params schema, with its JSON path.
fn param_descriptions(params: &serde_json::Value) -> Vec<(String, String)> {
    fn walk(value: &serde_json::Value, path: String, out: &mut Vec<(String, String)>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    match (key.as_str(), child) {
                        ("description", serde_json::Value::String(text)) => {
                            out.push((path.clone(), text.clone()))
                        }
                        _ => walk(child, format!("{path}.{key}"), out),
                    }
                }
            }
            serde_json::Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    walk(item, format!("{path}[{index}]"), out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(params, String::new(), &mut out);
    out
}

fn schema_texts(def: &crate::agent::club::ToolDef) -> Vec<String> {
    std::iter::once(def.description.clone())
        .chain(
            param_descriptions(&def.params)
                .into_iter()
                .map(|(_, text)| text),
        )
        .collect()
}

/// Every tool schema the harness advertises, full with its lean twin: the
/// practice registry in each mode that changes a schema (solo, remote consults
/// off, a research origin, a sealed task, YOLO), the tools registered only on
/// demand (language servers, the cockpit screen, embeddings, memory) and the
/// variants a mode picks (the full self map, the offline cargo).
fn every_schema(root: &std::path::Path) -> Vec<crate::agent::club::ToolDef> {
    use crate::agent::harness::Tool;
    use crate::tests::TestEnvGuard;
    // Schema coverage must include the optional vision tool on hosts without
    // credentials or a cached image-capable model catalog. No request is made.
    let _vision_url = TestEnvGuard::set("ANGEL_VISION_URL", "http://127.0.0.1:9/v1");
    let _vision_model = TestEnvGuard::set("ANGEL_VISION_MODEL", "schema-fixture");
    // Exercise the actual optional registration path without host credentials,
    // ambient enable/model settings, or a tool call. Guards restore the caller.
    let _jev_key = TestEnvGuard::set("TYPESAFE_API_KEY", "schema-fixture-key");
    let _jev_enabled = TestEnvGuard::set("ANGEL_JEV", "1");
    let _jev_model = TestEnvGuard::set("ANGEL_JEV_MODEL", "schema-fixture");
    // Grok's registration checks local file existence, not a token exchange.
    // The empty marker and missing catalog live only in the caller's scratch.
    let schema_grok_auth = root.join("schema-grok-auth.json");
    std::fs::write(&schema_grok_auth, "{}").unwrap();
    let _grok_auth = TestEnvGuard::set("ANGEL_GROK_OAUTH_FILE", schema_grok_auth.to_str().unwrap());
    let _grok_cache = TestEnvGuard::set(
        "ANGEL_GROK_MODELS_CACHE",
        root.join("schema-grok-models.json").to_str().unwrap(),
    );
    let _grok_cmd = TestEnvGuard::set("ANGEL_GROK_CMD", "/bin/false");
    let _grok_tool = TestEnvGuard::set("ANGEL_GROK_TOOL", "1");
    let _grok_research = TestEnvGuard::set("ANGEL_GROK_RESEARCH", "1");
    let _grok_model = TestEnvGuard::set("ANGEL_GROK_MODEL", "grok-4.7");
    let bag = crate::agent::club::Bag::practice_for_test();
    let mut defs = Vec::new();
    let modes: [&[(&'static str, &str)]; 3] = [
        &[],
        &[("ANGEL_SOLO", "1")],
        &[
            ("ANGEL_ALLOW_SOTA_CONSULT", "0"),
            ("ANGEL_SEARXNG_URL", "http://127.0.0.1:8888"),
            ("ANGEL_TASK_ACTIVE", "1"),
            ("ANGEL_TASK_SHELL_NO_DETACH", "1"),
            ("ANGEL_TASK_SHELL_PROTECT_GIT", "1"),
        ],
    ];
    for mode in modes {
        let _env: Vec<_> = mode
            .iter()
            .map(|(key, value)| TestEnvGuard::set(key, value))
            .collect();
        let (_history, registry) =
            crate::app::bootstrap::build_history_and_registry(&bag, "book-tools", root.into());
        defs.extend(registry.tools.iter().map(|tool| tool.def()));
    }
    let shell = crate::agent::tools::shell::ShellTool::in_dir(root.into());
    {
        let _task = TestEnvGuard::set("ANGEL_TASK_ACTIVE", "1");
        crate::platform::yolo::set(true);
        defs.push(shell.def());
    }
    defs.push(shell.def());
    crate::platform::yolo::set(false);
    crate::platform::yolo::set_smart(true);
    defs.push(shell.def());
    crate::platform::yolo::set_smart(false);
    {
        let _lsp = TestEnvGuard::set("ANGEL_LSP", "1");
        let (tools, _) = crate::agent::lsp::discover_lsp_tools(root.into());
        defs.extend(tools.iter().map(|tool| tool.def()));
    }
    let mut registry = crate::agent::harness::ToolRegistry::new();
    crate::ui::ui_inspect::install(&mut registry, crate::ui::ui_inspect::interactive_broker());
    {
        let _embed = TestEnvGuard::set("ANGEL_EMBED_URL", "http://127.0.0.1:9");
        crate::agent::harness::maybe_register_embedding_tools(&mut registry, root.into());
    }
    registry.register(Box::new(crate::agent::harness::RecallTool::new(
        crate::knowledge::memory::store::connect_from_env(),
        root,
    )));
    defs.extend(registry.tools.iter().map(|tool| tool.def()));
    defs.push(crate::agent::tools::self_model::SelfMapTool::new().def());
    defs.push(
        crate::agent::harness::CargoTool::in_dir(root.into())
            .offline()
            .def(),
    );
    std::fs::remove_file(schema_grok_auth).unwrap();
    defs
}

#[test]
fn given_optional_tools_absent_when_enumerating_then_schemas_and_env_survive() {
    use crate::tests::TestEnvGuard;
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-book-jev-schema-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    // Given no credential, disabled integration, or invalid ambient model,
    // When enumerating schemas, Then real registration is covered using only
    // synthetic fixture configuration and the original environment is restored.
    for (key, enabled, model) in [
        (None, "0", ""),
        (Some("  "), "1", "invalid/model"),
        (Some("synthetic-host-key"), "0", ""),
    ] {
        let _key = match key {
            Some(key) => TestEnvGuard::set("TYPESAFE_API_KEY", key),
            None => TestEnvGuard::unset("TYPESAFE_API_KEY"),
        };
        let _enabled = TestEnvGuard::set("ANGEL_JEV", enabled);
        let _model = TestEnvGuard::set("ANGEL_JEV_MODEL", model);
        let _grok_tool = TestEnvGuard::set("ANGEL_GROK_TOOL", "0");
        let _grok_research = TestEnvGuard::set("ANGEL_GROK_RESEARCH", "0");
        let missing = root.join("unconfigured-auth.json");
        let _grok_auth = TestEnvGuard::set("ANGEL_GROK_OAUTH_FILE", missing.to_str().unwrap());
        let defs = every_schema(&root);
        for tool in ["jev_decide", "grok_research"] {
            assert!(defs.iter().any(|def| def.name == tool), "{tool}");
        }
        assert_eq!(std::env::var("ANGEL_GROK_TOOL").unwrap(), "0");
        assert_eq!(std::env::var("ANGEL_GROK_RESEARCH").unwrap(), "0");
        assert_eq!(
            std::env::var("ANGEL_GROK_OAUTH_FILE").unwrap(),
            missing.to_str().unwrap()
        );
        assert!(!root.join("schema-grok-auth.json").exists());
        assert_eq!(std::env::var("TYPESAFE_API_KEY").ok().as_deref(), key);
        assert_eq!(std::env::var("ANGEL_JEV").unwrap(), enabled);
        assert_eq!(std::env::var("ANGEL_JEV_MODEL").unwrap(), model);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn every_schema_names_its_tools_section_and_carries_no_moved_sentence() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-book-tools-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let moved: Vec<&str> = ORIGINAL_TOOL_NOTES
        .iter()
        .flat_map(|(_, _, sentences)| sentences.iter().copied())
        .collect();
    let mut seen = std::collections::HashSet::new();
    for full in every_schema(&root) {
        let own = sections_of(&full.name);
        let lean = crate::agent::harness::lean_advertised_tool_def(&full);
        for def in [&full, &lean] {
            for text in schema_texts(def) {
                for sentence in &moved {
                    assert!(
                        !text.contains(sentence),
                        "{} still carries: {sentence}",
                        def.name
                    );
                }
            }
            // Its description names its section, or it has none.
            let routes = description_routes(def, &own);
            assert_eq!(
                routes.is_empty(),
                own.is_empty(),
                "{} ends with its route",
                def.name
            );
            // Every parameter description is its pages.
            for (path, text) in param_descriptions(&def.params) {
                let words: Vec<&str> = text.split_whitespace().collect();
                assert!(
                    words.first().is_some_and(|word| is_braille(word)),
                    "{}{path} is English: {text}",
                    def.name
                );
                check_pages_and_data(&def.name, &words, &own);
            }
        }
        // A lean parameter is the same page as its full one.
        let full_params: std::collections::HashMap<_, _> =
            param_descriptions(&full.params).into_iter().collect();
        for (path, text) in param_descriptions(&lean.params) {
            if let Some(original) = full_params.get(&path) {
                assert_eq!(&text, original, "lean {}{path}", full.name);
            }
        }
        seen.insert(full.name);
    }
    // Every Volume II section belongs to a tool the harness advertises.
    for (cells, tool, _) in ORIGINAL_TOOL_NOTES {
        let tool = tool.strip_suffix(CONTINUED).unwrap_or(tool);
        assert!(seen.contains(tool), "{cells} {tool} is no advertised tool");
    }
    let _ = std::fs::remove_dir_all(root);
}

/// Tool search indexes the English a schema's braille names: the route reads
/// as its section's pages, a parameter as its own.
#[test]
fn tool_search_reads_the_pages_a_schema_names() {
    use crate::agent::harness::Tool;
    let def = crate::agent::harness::FindFilesTool {
        root: std::path::PathBuf::from("."),
    }
    .def();
    let text = crate::agent::harness::tool_search_text(&def);
    assert!(
        text.contains("glob pattern, matched against workspace-relative paths"),
        "{text}"
    );
    assert!(
        text.contains("List workspace files matching a glob"),
        "{text}"
    );
    assert!(!text.chars().any(ledger::is_cell), "{text}");
}

/// Wire weight of the tool schemas: `cargo test schema_weight -- --ignored
/// --nocapture` prints bytes and English totals for the practice registry.
#[test]
#[ignore = "measurement"]
fn schema_weight() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-book-weight-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let bag = crate::agent::club::Bag::practice_for_test();
    let (_history, registry) =
        crate::app::bootstrap::build_history_and_registry(&bag, "book-weight", root.clone());
    let full: Vec<_> = registry.tools.iter().map(|tool| tool.def()).collect();
    let lean: Vec<_> = full
        .iter()
        .map(crate::agent::harness::lean_advertised_tool_def)
        .collect();
    let hot = registry.defs_for_run(None, true);
    for (label, defs) in [("full", &full), ("lean", &lean), ("lean hot path", &hot)] {
        let wire: Vec<_> = defs
            .iter()
            .map(|def| {
                serde_json::json!({"type": "function", "function": {
                    "name": def.name, "description": def.description, "parameters": def.params}})
            })
            .collect();
        let bytes = serde_json::to_string(&wire).unwrap().len();
        let descriptions: usize = defs.iter().map(|d| d.description.chars().count()).sum();
        let params: usize = defs
            .iter()
            .flat_map(|def| schema_texts(def).into_iter().skip(1))
            .map(|text| text.chars().count())
            .sum();
        println!(
            "{label}: {} tools, {bytes} bytes, descriptions {descriptions} chars, \
             param descriptions {params} chars",
            defs.len()
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

/// The advertised schemas as sent and with every braille description read out
/// in English (`ledger::expand`), written to `ANGEL_SCHEMA_DUMP/<set>-{braille,english}.json`
/// for an outside tokenizer: `ANGEL_SCHEMA_DUMP=/tmp/d cargo test schema_expansion_dump -- --ignored`.
#[test]
#[ignore = "measurement"]
fn schema_expansion_dump() {
    fn english(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(fields) => serde_json::Value::Object(
                fields
                    .iter()
                    .map(|(key, field)| {
                        let field = match (key.as_str(), field) {
                            ("description", serde_json::Value::String(text)) => {
                                serde_json::Value::String(super::super::ledger::expand(text))
                            }
                            _ => english(field),
                        };
                        (key.clone(), field)
                    })
                    .collect(),
            ),
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(english).collect())
            }
            other => other.clone(),
        }
    }
    let Some(dir) = std::env::var_os("ANGEL_SCHEMA_DUMP") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-book-dump-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let bag = crate::agent::club::Bag::practice_for_test();
    let (_history, registry) =
        crate::app::bootstrap::build_history_and_registry(&bag, "book-dump", root.clone());
    let full: Vec<_> = registry.tools.iter().map(|tool| tool.def()).collect();
    let lean: Vec<_> = full
        .iter()
        .map(crate::agent::harness::lean_advertised_tool_def)
        .collect();
    let hot = registry.defs_for_run(None, true);
    for (label, defs) in [("full", &full), ("lean", &lean), ("hot", &hot)] {
        let wire: Vec<_> = defs
            .iter()
            .map(|def| {
                serde_json::json!({"type": "function", "function": {
                    "name": def.name, "description": def.description, "parameters": def.params}})
            })
            .collect();
        let wire = serde_json::Value::Array(wire);
        std::fs::write(dir.join(format!("{label}-braille.json")), wire.to_string()).unwrap();
        std::fs::write(
            dir.join(format!("{label}-english.json")),
            english(&wire).to_string(),
        )
        .unwrap();
    }
    let _ = std::fs::remove_dir_all(root);
}
