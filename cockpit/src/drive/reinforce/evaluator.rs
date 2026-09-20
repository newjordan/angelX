//! Evaluator-owned policy execution boundary for receipt-backed promotion.

use super::{Candidate, EvaluatorEvidence};
use std::path::{Path, PathBuf};

const EVALUATOR_SPEC_SCHEMA: &str = "angel.rlvr.evaluator-spec/v2";
const INVENTORY_CONTRACT_SCHEMA: &str = "angel.rlvr.inventory-contract/v1";
const OUTCOME_REPORT_SCHEMA: &str = "angel.rlvr.test-outcome/v1";
const MAX_INVENTORY_BYTES: usize = 1024 * 1024;
const MAX_INVENTORY_ITEMS: usize = 16_384;
const MAX_INVENTORY_ITEM_BYTES: usize = 1024;

/// Framework-computed identity of the immutable inputs and execution contract
/// for one technical evaluator. Callers provide content digests, never a
/// handwritten aggregate identity string; this constructor validates and
/// domain-separates every field before the cohort is frozen.
#[derive(Clone, Debug)]
pub struct EvaluatorSpec {
    fixture_root: PathBuf,
    fixture_sha256: String,
    verifier_bundle_sha256: String,
    expected_inventory_sha256: String,
    parser_contract: String,
    command_sha256: String,
    inventory_command_sha256: String,
    execution_policy_sha256: String,
    kind: VerifierKind,
    resolved_tools: Vec<ResolvedTool>,
    manifest_sha256: String,
}

/// Which physical contract an evaluator binds. The kind is part of the spec
/// identity, so a case can never be evaluated under a contract it was not
/// frozen with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifierKind {
    /// A sealed libtest inventory plus a canonical per-test outcome report.
    LibtestInventory,
    /// A command-success objective: the operator's own verifier command, run by
    /// the evaluator, whose inventory observation is the pinned project/source
    /// identity. It never claims a test inventory was enumerated.
    CommandSuccess,
}

impl VerifierKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LibtestInventory => "libtest-inventory",
            Self::CommandSuccess => "command-success",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedTool {
    path: PathBuf,
    sha256: String,
}

impl EvaluatorSpec {
    pub fn new(
        fixture_root: &Path,
        expected_inventory_sha256: &str,
        parser_contract: &str,
        command: &str,
        inventory_command: &str,
        resolved_tools: &[&Path],
    ) -> Result<Self, String> {
        Self::build(
            VerifierKind::LibtestInventory,
            fixture_root,
            expected_inventory_sha256,
            parser_contract,
            command,
            inventory_command,
            resolved_tools,
            None,
        )
    }

    /// Command-success objective case: the operator's own verifier command,
    /// measured by evaluator-owned execution, over a frozen source identity.
    ///
    /// The inventory observation for this kind is the case identity itself —
    /// which source was frozen and which command verifies it — because an
    /// arbitrary operator command may enumerate no tests at all. Nothing here
    /// claims a libtest inventory, and the outcome reward reads the verifier's
    /// own exit status (plus its libtest summaries when it emits them).
    pub fn command_success(
        fixture_root: &Path,
        command: &str,
        resolved_tools: &[&Path],
    ) -> Result<Self, String> {
        let fixture_root = std::fs::canonicalize(fixture_root).map_err(|error| {
            format!(
                "could not resolve objective case fixture {}: {error}",
                fixture_root.display()
            )
        })?;
        let fixture_sha256 = crate::agent::harness::workspace_evidence_sha256(&fixture_root)
            .ok_or_else(|| "objective case fixture must be a Git-backed workspace".to_string())?;
        let command_sha256 = crate::knowledge::cut::sha256_hex(command.as_bytes());
        let mut identity = vec![
            format!("angel.rlvr.objective-case/v1 source {fixture_sha256}"),
            format!("angel.rlvr.objective-case/v1 verifier {command_sha256}"),
        ];
        identity.sort();
        let mut identity_bytes = String::new();
        for line in &identity {
            identity_bytes.push_str(line);
            identity_bytes.push('\n');
        }
        let expected_inventory_sha256 = canonical_inventory(identity_bytes.as_bytes())?.sha256;
        // The inventory command prints exactly those bytes; it is framework
        // authored and carries no candidate text.
        let inventory_command = format!("printf '%s' '{identity_bytes}'");
        Self::build(
            VerifierKind::CommandSuccess,
            &fixture_root,
            &expected_inventory_sha256,
            "objective-command/v1",
            command,
            &inventory_command,
            resolved_tools,
            Some(super::evaluator_objective_execution_policy_sha256()?),
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Keep each verifier identity field explicit at this private binding boundary"
    )]
    fn build(
        kind: VerifierKind,
        fixture_root: &Path,
        expected_inventory_sha256: &str,
        parser_contract: &str,
        command: &str,
        inventory_command: &str,
        resolved_tools: &[&Path],
        execution_policy_sha256: Option<String>,
    ) -> Result<Self, String> {
        for (label, digest) in [("expected inventory", expected_inventory_sha256)] {
            if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(format!(
                    "evaluator {label} digest must be 64 hexadecimal bytes"
                ));
            }
        }
        let parser_contract = parser_contract.trim();
        if parser_contract.is_empty() {
            return Err("evaluator parser contract must not be empty".into());
        }
        if command.trim().is_empty() {
            return Err("evaluator command must not be empty".into());
        }
        if inventory_command.trim().is_empty() {
            return Err("evaluator inventory command must not be empty".into());
        }
        let fixture_root = std::fs::canonicalize(fixture_root).map_err(|error| {
            format!(
                "could not resolve evaluator fixture {}: {error}",
                fixture_root.display()
            )
        })?;
        let fixture_sha256 = crate::agent::harness::workspace_evidence_sha256(&fixture_root)
            .ok_or_else(|| "evaluator fixture must be a Git-backed workspace".to_string())?;
        let expected_inventory_sha256 = expected_inventory_sha256.to_ascii_lowercase();
        let command_sha256 = crate::knowledge::cut::sha256_hex(command.as_bytes());
        let inventory_command_sha256 =
            crate::knowledge::cut::sha256_hex(inventory_command.as_bytes());
        let execution_policy_sha256 = match execution_policy_sha256 {
            Some(policy) => policy,
            None => super::evaluator_execution_policy_sha256()?,
        };
        let combined_commands = format!("{command}\n{inventory_command}");
        let mut resolved_tools = resolved_tools
            .iter()
            .map(|path| resolve_tool(path, &combined_commands))
            .collect::<Result<Vec<_>, _>>()?;
        resolved_tools.sort_by(|left, right| left.path.cmp(&right.path));
        if resolved_tools
            .windows(2)
            .any(|pair| pair[0].path == pair[1].path)
        {
            return Err("evaluator resolved tools must be unique".into());
        }
        // The argv-shape rules exist so a sealed libtest adapter cannot expand
        // its own command inline. A command-success case is the operator's own
        // verifier: its boundary is evaluator-owned sandboxed execution (writable
        // roots fixed, network denied), not the shape of its argv, so the
        // command is only bound into the verifier identity below.
        if kind == VerifierKind::LibtestInventory {
            validate_simple_invocation(command, &resolved_tools)?;
            validate_simple_invocation(inventory_command, &resolved_tools)?;
            let declared = resolved_tools
                .iter()
                .map(|tool| tool.path.as_path())
                .collect::<std::collections::HashSet<_>>();
            for path in absolute_executable_paths(&combined_commands) {
                if !declared.contains(path.as_path()) {
                    return Err(format!(
                        "evaluator command uses undeclared absolute tool: {}",
                        path.display()
                    ));
                }
            }
        }
        let mut canonical_verifier = String::new();
        for value in [parser_contract, command, inventory_command] {
            canonical_verifier.push_str(&value.len().to_string());
            canonical_verifier.push(':');
            canonical_verifier.push_str(value);
            canonical_verifier.push('\n');
        }
        for tool in &resolved_tools {
            let path = tool.path.to_string_lossy();
            for value in [path.as_ref(), tool.sha256.as_str()] {
                canonical_verifier.push_str(&value.len().to_string());
                canonical_verifier.push(':');
                canonical_verifier.push_str(value);
                canonical_verifier.push('\n');
            }
        }
        let verifier_bundle_sha256 =
            crate::knowledge::cut::sha256_hex(canonical_verifier.as_bytes());
        let mut canonical = String::new();
        for value in [
            EVALUATOR_SPEC_SCHEMA,
            kind.as_str(),
            &fixture_sha256,
            &verifier_bundle_sha256,
            &expected_inventory_sha256,
            parser_contract,
            &command_sha256,
            &inventory_command_sha256,
            &execution_policy_sha256,
        ] {
            canonical.push_str(&value.len().to_string());
            canonical.push(':');
            canonical.push_str(value);
            canonical.push('\n');
        }
        for tool in &resolved_tools {
            let path = tool.path.to_string_lossy();
            for value in [path.as_ref(), tool.sha256.as_str()] {
                canonical.push_str(&value.len().to_string());
                canonical.push(':');
                canonical.push_str(value);
                canonical.push('\n');
            }
        }
        Ok(Self {
            fixture_root,
            fixture_sha256,
            verifier_bundle_sha256,
            expected_inventory_sha256,
            parser_contract: parser_contract.to_string(),
            command_sha256,
            inventory_command_sha256,
            execution_policy_sha256,
            kind,
            resolved_tools,
            manifest_sha256: crate::knowledge::cut::sha256_hex(canonical.as_bytes()),
        })
    }

    /// Which physical contract this case binds.
    pub fn kind(&self) -> VerifierKind {
        self.kind
    }

    /// The frozen fixture this case measures against.
    pub fn fixture_root(&self) -> &Path {
        &self.fixture_root
    }

    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    pub fn command_sha256(&self) -> &str {
        &self.command_sha256
    }

    pub fn inventory_command_sha256(&self) -> &str {
        &self.inventory_command_sha256
    }

    pub fn expected_inventory_sha256(&self) -> &str {
        &self.expected_inventory_sha256
    }

    pub fn fixture_sha256(&self) -> &str {
        &self.fixture_sha256
    }

    pub fn verifier_bundle_sha256(&self) -> &str {
        &self.verifier_bundle_sha256
    }

    pub fn outcome_contract(&self) -> &str {
        self.manifest_sha256()
    }

    pub fn inventory_contract(&self) -> String {
        format!("{INVENTORY_CONTRACT_SCHEMA}\n{}", self.manifest_sha256())
    }

    pub fn execution_policy_sha256(&self) -> &str {
        &self.execution_policy_sha256
    }

    pub fn validate_runtime(&self) -> Result<(), String> {
        let fixture_sha256 = crate::agent::harness::workspace_evidence_sha256(&self.fixture_root)
            .ok_or_else(|| {
            "evaluator fixture is no longer a Git-backed workspace".to_string()
        })?;
        if fixture_sha256 != self.fixture_sha256 {
            return Err(format!(
                "evaluator fixture drift: expected {}, observed {fixture_sha256}",
                self.fixture_sha256
            ));
        }
        for expected in &self.resolved_tools {
            let actual = resolved_tool_identity(&expected.path)?;
            if &actual != expected {
                return Err(format!(
                    "evaluator resolved tool drift: {}",
                    expected.path.display()
                ));
            }
        }
        Ok(())
    }
}

fn resolve_tool(path: &Path, command: &str) -> Result<ResolvedTool, String> {
    if !path.is_absolute() {
        return Err(format!(
            "evaluator tool path must be absolute: {}",
            path.display()
        ));
    }
    let tool = resolved_tool_identity(path)?;
    let _ = command;
    Ok(tool)
}

fn validate_simple_invocation(
    command: &str,
    resolved_tools: &[ResolvedTool],
) -> Result<(), String> {
    if command.contains(|character: char| {
        character.is_control()
            || character.is_whitespace() && character != ' '
            || matches!(
                character,
                '\'' | '"'
                    | '`'
                    | '$'
                    | ';'
                    | '&'
                    | '|'
                    | '<'
                    | '>'
                    | '('
                    | ')'
                    | '\\'
                    | '*'
                    | '?'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '~'
                    | '!'
            )
    }) {
        return Err("evaluator command must be a simple, non-expanding invocation".into());
    }
    let tokens = command
        .split(' ')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let executable = tokens
        .first()
        .ok_or_else(|| "evaluator command must not be empty".to_string())?;
    if !executable.starts_with('/') {
        return Err("evaluator command executable must be an absolute declared tool".into());
    }
    let executable = std::fs::canonicalize(executable).map_err(|error| {
        format!("could not resolve evaluator command executable {executable}: {error}")
    })?;
    if tokens[0] != executable.to_string_lossy() {
        return Err("evaluator command executable must use its canonical path".into());
    }
    if !resolved_tools.iter().any(|tool| tool.path == executable) {
        return Err(format!(
            "evaluator command executable is not a declared tool: {}",
            executable.display()
        ));
    }
    let launcher = executable
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if matches!(
        launcher,
        "sh" | "dash" | "bash" | "env" | "python" | "python3" | "node" | "perl" | "ruby"
    ) {
        return Err("evaluator command may not use a generic interpreter/launcher".into());
    }
    for token in tokens.iter().skip(1).filter(|token| token.starts_with('/')) {
        let path = std::fs::canonicalize(token).map_err(|error| {
            format!("could not resolve evaluator command path {token}: {error}")
        })?;
        if *token != path.to_string_lossy() {
            return Err("evaluator command arguments must use canonical paths".into());
        }
        if !resolved_tools.iter().any(|tool| tool.path == path) {
            return Err(format!(
                "evaluator command path is not a declared tool: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn resolved_tool_identity(path: &Path) -> Result<ResolvedTool, String> {
    let path = std::fs::canonicalize(path).map_err(|error| {
        format!(
            "could not resolve evaluator tool {}: {error}",
            path.display()
        )
    })?;
    if !path.is_file() {
        return Err(format!("evaluator tool is not a file: {}", path.display()));
    }
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("could not hash evaluator tool {}: {error}", path.display()))?;
    Ok(ResolvedTool {
        path,
        sha256: crate::knowledge::cut::sha256_hex(&bytes),
    })
}

pub(super) fn absolute_executable_paths(command: &str) -> Vec<PathBuf> {
    let mut paths = command
        .split(|character: char| {
            character.is_whitespace()
                || matches!(
                    character,
                    '\'' | '"'
                        | '`'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | ';'
                        | '&'
                        | '|'
                        | '<'
                        | '>'
                        | '$'
                )
        })
        .filter(|token| token.starts_with('/'))
        .filter_map(|token| std::fs::canonicalize(token).ok())
        .filter(|path| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::metadata(path).is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                })
            }
            #[cfg(not(unix))]
            {
                path.is_file()
            }
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

/// Exact identity of one generated sample presented to an evaluator. The
/// canonical subject is bound into the returned receipt and checked again by
/// promotion before a reward can be counted.
pub struct PolicyEvaluationRequest<'a> {
    pub cohort_manifest_sha256: &'a str,
    pub cohort_role: &'a str,
    pub case_id: &'a str,
    pub task: &'a str,
    pub prompt_sha256: &'a str,
    pub policy_version: u64,
    pub sample_index: usize,
    pub candidate: &'a Candidate,
}

impl PolicyEvaluationRequest<'_> {
    pub fn canonical_subject(&self) -> String {
        self.canonical_subject_for("outcome")
    }

    pub fn canonical_inventory_subject(&self) -> String {
        self.canonical_subject_for("inventory")
    }

    /// Stable logical sample slot used for durable pair consumption. Candidate
    /// output is deliberately excluded: a stochastic rerun of the same
    /// arm/case/sample must collide with the original slot rather than creating
    /// another optional-stopping opportunity.
    pub fn canonical_pair_id(&self) -> String {
        let mut slot = String::new();
        for value in [
            self.cohort_manifest_sha256.to_string(),
            self.cohort_role.to_string(),
            self.case_id.to_string(),
            crate::knowledge::cut::sha256_hex(self.task.as_bytes()),
            self.prompt_sha256.to_string(),
            self.policy_version.to_string(),
            self.sample_index.to_string(),
        ] {
            slot.push_str(&value.len().to_string());
            slot.push(':');
            slot.push_str(&value);
            slot.push('\n');
        }
        crate::knowledge::cut::sha256_hex(slot.as_bytes())
    }

    fn canonical_subject_for(&self, receipt_role: &str) -> String {
        let mut subject = String::new();
        for value in [
            receipt_role.to_string(),
            self.cohort_manifest_sha256.to_string(),
            self.cohort_role.to_string(),
            self.case_id.to_string(),
            crate::knowledge::cut::sha256_hex(self.task.as_bytes()),
            self.prompt_sha256.to_string(),
            self.policy_version.to_string(),
            self.sample_index.to_string(),
            crate::knowledge::cut::sha256_hex(self.candidate.output.as_bytes()),
        ] {
            subject.push_str(&value.len().to_string());
            subject.push(':');
            subject.push_str(&value);
            subject.push('\n');
        }
        subject
    }
}

/// The two independent physical observations required for one technical
/// sample. Inventory proves which tests exist; outcome proves how they ran.
/// Purpose-specific subjects and verifier contracts prevent substitution.
pub struct PolicyEvaluationEvidence {
    pub inventory: EvaluatorEvidence,
    pub outcome: EvaluatorEvidence,
}

/// Trusted evaluator implementation used for technical promotion cases.
///
/// `spec` binds the frozen fixture, verifier bundle, expected inventory,
/// parser, command, and execution policy into a framework-computed manifest.
pub trait PolicyEvaluator: Send + Sync {
    fn spec(&self) -> &EvaluatorSpec;
    fn evaluate(
        &self,
        request: &PolicyEvaluationRequest<'_>,
    ) -> Result<PolicyEvaluationEvidence, String>;
}

/// Validate the canonical inventory wire format and return its exact digest.
/// IDs must be strict UTF-8, unique, byte-sorted, and terminated by one newline.
/// No trimming or normalization is performed, so prose/status output cannot be
/// mistaken for the frozen expected inventory.
pub(crate) struct CanonicalInventory {
    pub sha256: String,
    pub item_count: usize,
}

pub(crate) struct CanonicalTestOutcome {
    pub inventory_sha256: String,
    pub passed: usize,
    pub failed: usize,
}

pub(crate) fn canonical_inventory(bytes: &[u8]) -> Result<CanonicalInventory, String> {
    if bytes.is_empty() || bytes.len() > MAX_INVENTORY_BYTES {
        return Err("evaluator inventory is empty or exceeds its byte limit".into());
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "evaluator inventory must be strict UTF-8".to_string())?;
    if !text.ends_with('\n') || text.contains('\r') || text.contains('\0') {
        return Err("evaluator inventory must use canonical LF-terminated lines".into());
    }
    let items = text[..text.len() - 1].split('\n').collect::<Vec<_>>();
    if items.is_empty() || items.len() > MAX_INVENTORY_ITEMS {
        return Err("evaluator inventory item count is invalid".into());
    }
    for item in &items {
        if item.is_empty()
            || item.len() > MAX_INVENTORY_ITEM_BYTES
            || item.trim() != *item
            || item.chars().any(inventory_character_is_forbidden)
        {
            return Err("evaluator inventory contains a non-canonical test id".into());
        }
    }
    if items.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("evaluator inventory test ids must be unique and byte-sorted".into());
    }
    Ok(CanonicalInventory {
        sha256: crate::knowledge::cut::sha256_hex(bytes),
        item_count: items.len(),
    })
}

fn inventory_character_is_forbidden(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

/// Parse the only technical outcome format accepted by the production gate.
/// A pinned evaluator adapter—not candidate test output—must emit this exact
/// record, one byte-sorted `test-id<TAB>pass|fail` row per executed test.
pub(crate) fn canonical_test_outcome(bytes: &[u8]) -> Result<CanonicalTestOutcome, String> {
    if bytes.is_empty() || bytes.len() > MAX_INVENTORY_BYTES {
        return Err("evaluator outcome report is empty or exceeds its byte limit".into());
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "evaluator outcome report must be strict UTF-8".to_string())?;
    if !text.ends_with('\n') || text.contains('\r') || text.contains('\0') {
        return Err("evaluator outcome report must use canonical LF-terminated lines".into());
    }
    let mut lines = text[..text.len() - 1].split('\n');
    if lines.next() != Some(OUTCOME_REPORT_SCHEMA) {
        return Err("evaluator outcome report schema is missing or invalid".into());
    }
    let mut inventory = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    for line in lines {
        let (id, status) = line
            .rsplit_once('\t')
            .ok_or_else(|| "evaluator outcome row is malformed".to_string())?;
        match status {
            "pass" => passed = passed.saturating_add(1),
            "fail" => failed = failed.saturating_add(1),
            _ => return Err("evaluator outcome status must be pass or fail".into()),
        }
        inventory.extend_from_slice(id.as_bytes());
        inventory.push(b'\n');
    }
    let canonical = canonical_inventory(&inventory)?;
    if canonical.item_count != passed.saturating_add(failed) {
        return Err("evaluator outcome report count is internally inconsistent".into());
    }
    Ok(CanonicalTestOutcome {
        inventory_sha256: canonical.sha256,
        passed,
        failed,
    })
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/reinforce/evaluator__tests.rs"]
mod tests;
