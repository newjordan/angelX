use crate::agent::club::{Bag, ChatMsg, ChatRole, Club, is_sota_label};
use crate::agent::harness::{self, ToolRegistry};
use crate::agent::{lsp, mcp};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) struct StartupContext {
    pub(crate) history: Vec<ChatMsg>,
    pub(crate) tools: Arc<ToolRegistry>,
}

pub(crate) fn build(bag: &Bag, session_id: &str) -> StartupContext {
    // The interactive TUI operates on the directory it was launched in (like
    // Claude Code), overridable by `$ANGEL_WORKSPACE`. `/cd` swaps it later.
    let workspace = harness::resolve_workspace(None, harness::current_dir_workspace);
    let (history, mut registry) = build_history_and_registry(bag, session_id, workspace);
    crate::ui::ui_inspect::install(&mut registry, crate::ui::ui_inspect::interactive_broker());
    // This is the only registry that drives the live TUI. Delegates, worktrees,
    // task mode, and evaluation registries stay deliberately unattended even if
    // the action-capsule environment knob is set process-wide.
    registry.enable_action_capsules();
    StartupContext {
        history,
        tools: Arc::new(registry),
    }
}

/// Static harness policy. Repository guidance and learned evidence are separate
/// Harness-role messages, never additions to provider System authority.
pub(crate) fn build_system_prompt(bag: &Bag, workspace: &Path) -> String {
    let roster = delegation_roster(bag);
    let specialists = specialist_labels(&roster, bag.in_hand_label());
    // Environment capabilities, verified at prompt build, are the `⠝⠁`
    // evidence. A model that has to guess what exists here guesses wrong in
    // both directions — inventing image generation, declaring installs
    // impossible — and both failure modes cost whole turns.
    let withheld = withheld_sota_labels(bag, &roster);
    // Lane routes: Treebeard (RLM/HiQ). Static for the process env, so the
    // entry warpath stays on the prompt-cache prefix.
    let lanes: Vec<harness::book::Route> = harness::lane_route().into_iter().collect();
    let mut system = harness::book::y_types::cockpit_entry(
        workspace,
        &harness::delegate_roles(&specialists),
        &capabilities_block(&withheld),
        &lanes,
    );
    // DeepSeek Flash (and other text-only drivers): eyes via ANGEL_VISION_* sidecar.
    if let Some(sign) =
        crate::agent::tools::vision::vision_sidecar_prompt_hint(bag.in_hand().as_ref())
    {
        system.push('\n');
        system.push_str(&sign);
    }
    system
}

pub(crate) const WORKSPACE_CONTEXT_HEADER: &str = "[workspace-context/v1]";
/// JSON escaping keeps artifact text inside its named source field; the carrier
/// remains distinct from actual operator User messages throughout compaction.
pub(crate) fn workspace_context_message(
    workspace: &Path,
    project_guidance: String,
    skills_catalog: String,
    evidence: String,
) -> Option<ChatMsg> {
    // Even a fresh, empty project needs an unambiguous working-directory anchor.
    let payload = serde_json::json!({
        "workspace": workspace.to_string_lossy(),
        "project_guidance": project_guidance,
        "skills_catalog": skills_catalog,
        "evidence": evidence,
    });
    Some(ChatMsg::harness(format!(
        "{WORKSPACE_CONTEXT_HEADER}\n{payload}"
    )))
}

pub(crate) fn is_workspace_context(message: &ChatMsg) -> bool {
    message.role == ChatRole::Harness && message.content.starts_with(WORKSPACE_CONTEXT_HEADER)
}

/// Read the harness-produced byte marker from the guidance field only. A
/// quoted marker in dossier/skill evidence must not alter prompt telemetry.
/// Oversized or malformed imported envelopes have no trusted byte measurement.
pub(crate) fn project_doc_bytes(message: &ChatMsg) -> Option<usize> {
    const MAX_ENVELOPE_BYTES: usize = 1024 * 1024;
    if !is_workspace_context(message) || message.content.len() > MAX_ENVELOPE_BYTES {
        return None;
    }
    let payload = message
        .content
        .strip_prefix(WORKSPACE_CONTEXT_HEADER)?
        .strip_prefix('\n')?;
    let payload: serde_json::Value = serde_json::from_str(payload).ok()?;
    let guidance = payload.get("project_guidance")?.as_str()?;
    let count = guidance
        .trim_start()
        .strip_prefix(harness::PROJECT_DOC_BYTES_MARKER)?;
    count.split_whitespace().next()?.parse().ok()
}

pub(crate) fn is_pinned_preamble(message: &ChatMsg) -> bool {
    (message.role == ChatRole::System && !crate::agent::compaction::is_compaction_note(message))
        || is_workspace_context(message)
}

fn build_history_with_skills(
    bag: &Bag,
    workspace: &Path,
    skills: &[harness::Skill],
) -> Vec<ChatMsg> {
    let mut history = vec![ChatMsg::system(build_system_prompt(bag, workspace))];
    let mut evidence = crate::agent::tools::work_landing::work_context_block(workspace);
    if !crate::agent::backplane::active() {
        let dossier = crate::knowledge::dossier::context_block(workspace);
        if !dossier.is_empty() {
            evidence.push_str(&crate::knowledge::evidence::fence(
                "dossier",
                &format!(
                    "repo={} age=unknown verification=belief-gated",
                    crate::platform::workspace_store::repo_identity(workspace).key
                ),
                &dossier,
            ));
        }
        let card = crate::knowledge::caddy::render_card_for_task(
            workspace,
            crate::knowledge::caddy::card_cap(),
        );
        if !card.is_empty() {
            evidence.push_str(&crate::knowledge::evidence::fence(
                "caddy",
                &format!(
                    "repo={} age=unknown verification=store-claimed",
                    crate::platform::workspace_store::repo_identity(workspace).key
                ),
                &card,
            ));
        }
    }
    evidence.push_str(&crate::drive::continual_harness::context_block(workspace));
    evidence.push_str(&crate::agent::tools::self_model::self_context(workspace));
    if let Some(context) = workspace_context_message(
        workspace,
        harness::project_context(workspace),
        harness::skills_catalog(skills),
        evidence,
    ) {
        history.push(context);
    }
    history
}

/// The same explicit role split used by the real headless CLI. Keeping the
/// assembly here lets offline wire tests exercise the production message list.
pub(crate) fn build_task_history(
    specialists: &[String],
    skills: &[harness::Skill],
    workspace: &Path,
    vision_hint: Option<&str>,
) -> Vec<ChatMsg> {
    let compact = harness::task_compact_prompt_enabled();
    let mut system = harness::task_system_prompt(workspace, specialists);
    if let Some(sign) = vision_hint {
        system.push('\n');
        system.push_str(sign);
    }
    let mut history = vec![ChatMsg::system(system)];
    // Lean referral (rides the compact flag): the full skill catalog lists
    // every name up front; the referral keeps one line and lets the `skill`
    // tool surface names/descriptions on demand instead.
    let skills_text = if compact {
        harness::skills_referral_card(skills)
    } else {
        harness::skills_catalog(skills)
    };
    history.extend(workspace_context_message(
        workspace,
        harness::project_context(workspace),
        skills_text,
        harness::task_warm_start(workspace),
    ));
    history
}

#[cfg(test)]
mod compact_prompt_tests {
    use super::*;

    /// The task system prompt is the entry warpath — personality type, pace,
    /// repair discipline — and the workspace map as data; the compact flag
    /// picks the compact type.
    #[test]
    fn task_system_prompt_is_the_entry_warpath_and_data() {
        let tmp = std::env::temp_dir().join("angel-compact-prompt-test");
        std::fs::create_dir_all(&tmp).unwrap();
        let specialists: Vec<String> = vec!["local".into(), "swarm".into()];
        let _guard = crate::tests::env_lock();
        let _pace = crate::tests::TestEnvGuard::set("ANGEL_TASK_PACE_RESOLVED", "rapid");
        let _map = crate::tests::TestEnvGuard::unset("ANGEL_TASK_WORKSPACE_MAP");
        let _repair = crate::tests::TestEnvGuard::set("ANGEL_TASK_CODING_DISCIPLINE", "1");
        let system = |compact: &str| {
            let _flag = crate::tests::TestEnvGuard::set("ANGEL_TASK_COMPACT_PROMPT", compact);
            build_task_history(&specialists, &[], &tmp, None)[0]
                .content
                .to_string()
        };
        assert_eq!(system("0"), "⠽⠃⠍⠁⠽⠑", "full Driver with teammates");
        assert_eq!(system("1"), "⠽⠙⠍⠁⠽⠑", "compact core with teammates");
        let routes = harness::book::ledger::read(&tmp, "⠕⠋").unwrap();
        assert!(
            routes.contains("- local:") && routes.contains("- swarm:"),
            "{routes}"
        );
    }
}

pub(crate) fn build_history(bag: &Bag, workspace: &Path) -> Vec<ChatMsg> {
    let skills = harness::load_skills_for(workspace);
    build_history_with_skills(bag, workspace, &skills)
}

/// Replace only bootstrap authority and tagged workspace data. Preserve every
/// actual operator message, tool pair, and legacy summary; legacy System
/// summaries are demoted to Harness-role background before being retained.
pub(crate) fn refresh_history(bag: &Bag, workspace: &Path, history: &mut Vec<ChatMsg>) {
    let mut retained = std::mem::take(history);
    retained.retain_mut(|message| {
        if is_workspace_context(message)
            || (message.role == ChatRole::Harness
                && (crate::agent::backplane::is_broker_message(&message.content)
                    || message
                        .content
                        .starts_with(harness::AUTO_RECALL_NOTE_PREFIX)))
        {
            return false;
        }
        if message.role == ChatRole::System {
            if crate::agent::compaction::is_compaction_note(message) {
                message.role = ChatRole::Harness;
                return true;
            }
            return false;
        }
        true
    });
    *history = build_history(bag, workspace);
    history.append(&mut retained);
}

/// Build the full orchestrator tool registry rooted at `workspace`: the team kit
/// (scoped file/build/shell + delegate/integrate), skills, self-map, MCP, LSP, and
/// the long-form memory store, with the session id stamped for drawer provenance.
///
/// Used at startup AND by `/cd`, which rebuilds the registry at a new root and
/// swaps `App.tools`. Dropping the previous `Arc` runs each MCP/LSP client's
/// `Drop` (which kills its server subprocess), so re-scoping needs no manual
/// teardown.
pub(crate) fn build_registry(bag: &Bag, session_id: &str, workspace: PathBuf) -> ToolRegistry {
    let skills = harness::load_skills_for(&workspace);
    build_registry_with_skills(bag, session_id, workspace, skills)
}

/// Build the two workspace-bound startup surfaces from one admitted skill
/// catalog. Interactive startup and `/cd` both need the prompt and registry;
/// sharing the snapshot avoids parsing every compatible skill root twice while
/// preserving the standalone builders used by resume/import and self-worktrees.
pub(crate) fn build_history_and_registry(
    bag: &Bag,
    session_id: &str,
    workspace: PathBuf,
) -> (Vec<ChatMsg>, ToolRegistry) {
    let skills = harness::load_skills_for(&workspace);
    let history = build_history_with_skills(bag, &workspace, &skills);
    let registry = build_registry_with_skills(bag, session_id, workspace, skills);
    (history, registry)
}

fn build_registry_with_skills(
    bag: &Bag,
    session_id: &str,
    workspace: PathBuf,
    skills: Vec<harness::Skill>,
) -> ToolRegistry {
    let roster = delegation_roster(bag);
    let mut registry = ToolRegistry::with_team_self(workspace.clone(), roster, Some(bag.in_hand()));
    registry.set_backplane(crate::agent::backplane::BackplaneRegistry::from_bag(bag));
    registry.set_skill_index(&skills);
    if !skills.is_empty() {
        registry.register(Box::new(harness::SkillTool::for_workspace(
            skills,
            workspace.clone(),
        )));
    }
    // Explicit self-diagnostics remain available on demand. Bind self-work to
    // this checkout; an unrelated project never gains a write path into the install.
    registry.register(Box::new(
        crate::agent::tools::self_model::SelfMapTool::for_workspace(&workspace),
    ));
    // `work_landing` — first-contact onboarding: detect folder/repo/visibility,
    // confirm with the user, and record the behavior mode for this workspace.
    registry.register(Box::new(
        crate::agent::tools::work_landing::WorkLandingTool::new(workspace.clone()),
    ));
    install_mcp_tools(&mut registry, &workspace);
    install_lsp_tools(&mut registry, &workspace);
    // Long-form memory ("palace"): auto-compaction deposits distilled notes here
    // and recall reads them back. No-op unless `ANGEL_MEMPALACE_CMD` is set; only
    // then is the `recall` tool offered (no point advertising it with no backend).
    let store = crate::knowledge::memory::store::connect_from_env();
    if store.is_live() {
        registry.register(Box::new(harness::RecallTool::new(
            Arc::clone(&store),
            &workspace,
        )));
    }
    // Refresh after every built-in, MCP, LSP, skill, and memory tool is present.
    // Lean schema profiles can then discover anything they did not advertise.
    registry.enable_tool_search();
    registry.set_memory_store(store);
    // Stamp the session id so every drawer deposited this run is attributable to
    // it (see `compaction::provenance`).
    registry.set_session_id(session_id);
    // Local plain-model clubs for background utility work (compaction summaries),
    // so bulk summarization never lands on a paid SOTA driver or fans out through
    // a pipeline club when a local fleet model can do it.
    registry.set_aux_clubs(utility_roster(bag));
    registry
}

/// Local, plain-model clubs suitable for background utility work: never a
/// token-metered SOTA link, never the practice echo (its "summaries" are just
/// the prompt echoed back), never a fan-out pipeline like the swarm (each
/// summary call would become a whole MoA run). Bag order — the fast MoE (gemma)
/// leads on the standard fleet, which is exactly the summarizer you want.
pub(crate) fn utility_roster(bag: &Bag) -> Vec<Arc<dyn Club>> {
    bag.roster()
        .into_iter()
        .filter(|c| !is_sota_label(c.label()) && c.label() != "practice" && !c.reports_to_palace())
        .collect()
}

/// SOTA labels present in the bag but stripped from the delegation roster when
/// the operator sets `ANGEL_ALLOW_SOTA_DELEGATE=0`. Named in the capability
/// manifest so "not offered" reads as "opted out", never as "does not exist".
fn withheld_sota_labels(bag: &Bag, roster: &[Arc<dyn Club>]) -> Vec<String> {
    let offered: Vec<&str> = roster.iter().map(|club| club.label()).collect();
    bag.roster()
        .iter()
        .map(|club| club.label().to_string())
        .filter(|label| !offered.contains(&label.as_str()) && label != bag.in_hand_label())
        .collect()
}

/// Binaries worth stating up front: the ones agent tasks reach for and models
/// most often wrongly assume present or absent. Everything else stays a
/// `command -v` probe away.
const PROBED_BINARIES: [&str; 15] = [
    "git", "node", "npm", "bun", "python3", "pip3", "uv", "cargo", "ffmpeg", "docker", "gh", "rg",
    "jq", "curl", "make",
];

fn path_has(bin: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(bin).is_file()))
}

fn capabilities_block(withheld_sota: &[String]) -> String {
    let (present, absent): (Vec<&str>, Vec<&str>) =
        PROBED_BINARIES.iter().partition(|bin| path_has(bin));
    // The header and the closing rule are the `⠝⠁` pages; these lines are what
    // this session found.
    let mut block = format!("- On PATH: {}\n", present.join(", "));
    if !absent.is_empty() {
        block.push_str(&format!(
            "- Not on PATH: {} (self-serve user-level if a task needs one)\n",
            absent.join(", ")
        ));
    }
    if crate::agent::sandbox::enabled() {
        block.push_str(
            "- Installs: privilege escalation is disabled, so sudo/system package managers can \
             never work — do not conclude installs are impossible. User-level installs do work: \
             pip/npm/cargo, or a static binary dropped into ~/.local/bin (writable, on PATH). \
             Network is available.\n",
        );
    }
    if let Some(reason) = crate::agent::club::grok_research_unavailable_reason() {
        block.push_str(&format!("- grok_research tool: unavailable — {reason}\n"));
    }
    if crate::agent::tools::jev::available() {
        block.push_str("- Jev advisory decisions: discover jev_decide with tool_search for batched technical probabilities, defect triage and experiment hypotheses. Use benchmark_compare for measured percentages; neither grants evaluator acceptance.\n");
    }
    if !withheld_sota.is_empty() {
        block.push_str(&format!(
            "- SOTA clubs withheld from spawn/delegate (operator set ANGEL_ALLOW_SOTA_DELEGATE=0): \
             {} — unset that pin or set ANGEL_ALLOW_SOTA_DELEGATE=1 to restore them\n",
            withheld_sota.join(", ")
        ));
    }
    block
}

pub(crate) fn delegation_roster(bag: &Bag) -> Vec<Arc<dyn Club>> {
    // SOTA seats are first-class agents. Delegation/spawn includes them by
    // default; operators who want a local-only fleet opt out with
    // ANGEL_ALLOW_SOTA_DELEGATE=0.
    let allow_sota = crate::agent::harness::env_flag("ANGEL_ALLOW_SOTA_DELEGATE", true);
    filter_delegation_roster(bag.roster(), allow_sota)
}

fn filter_delegation_roster(roster: Vec<Arc<dyn Club>>, allow_sota: bool) -> Vec<Arc<dyn Club>> {
    roster
        .into_iter()
        .filter(|club| allow_sota || !is_sota_label(club.label()))
        .collect()
}

fn specialist_labels(roster: &[Arc<dyn Club>], in_hand_label: &str) -> Vec<String> {
    roster
        .iter()
        .map(|club| club.label().to_string())
        .filter(|label| label != in_hand_label && label != "practice")
        .collect()
}

fn install_mcp_tools(registry: &mut ToolRegistry, workspace: &Path) {
    let (compositions, _mcp_notes) = mcp::discover_mcp_compositions(workspace);
    let total: usize = compositions
        .iter()
        .map(|composition| composition.tools.len())
        .sum();
    let deferred = total > 6;
    // A server is one capability with N registrations: each tool carries the
    // inverse the registry fires when it is retracted, and the provider unloads
    // when the last of them goes (paper §5.1.1 parent composition). That is what
    // makes a mid-session drop possible: `unregister_prefix("server__")` retracts
    // the tools and reaps the child without restarting the cockpit.
    for composition in compositions {
        let provider = composition.server.clone();
        let (tools, teardown) = composition.into_parts();
        if tools.is_empty() {
            teardown();
            continue;
        }
        // §5.1.3 inertial teardown, in the order it asks for: release the
        // provider's coeffect binding first, so every dependent is reclassified
        // and its schema leaves the advertisement, and only then let the process
        // go. The lease's release and the process teardown are one inverse; a
        // mid-session `unregister_prefix` walks the same path.
        let lease = registry.bind_provider(&provider);
        let scope =
            crate::agent::harness::registration::ProviderScope::new(tools.len(), move || {
                lease.release();
                teardown();
            });
        for tool in tools {
            registry.track_registration_in_provider(
                tool,
                scope.registration(),
                &provider,
                deferred,
            );
        }
    }
    if deferred {
        registry.enable_tool_search();
    }
}

fn install_lsp_tools(registry: &mut ToolRegistry, workspace: &Path) {
    let (lsp_tools, _lsp_notes) = lsp::discover_lsp_tools(workspace.to_path_buf());
    for tool in lsp_tools {
        registry.register(tool);
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/bootstrap__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/bootstrap_provenance_tests.rs"]
mod provenance_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/bootstrap_mcp_provider_tests.rs"]
mod mcp_provider_tests;
