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
    let workspace = harness::resolve_workspace(None, harness::current_dir_workspace);
    build_in_workspace(bag, session_id, workspace)
}

pub(crate) fn build_in_workspace(
    bag: &Bag,
    session_id: &str,
    workspace: PathBuf,
) -> StartupContext {
    let (history, mut registry) = build_history_and_registry(bag, session_id, workspace);
    crate::ui::ui_inspect::install(&mut registry, crate::ui::ui_inspect::interactive_broker());
    registry.enable_action_capsules();
    StartupContext {
        history,
        tools: Arc::new(registry),
    }
}

pub(crate) fn build_system_prompt(bag: &Bag, workspace: &Path) -> String {
    let roster = delegation_roster(bag);
    let specialists = specialist_labels(&roster, bag.in_hand_label());
    let withheld = withheld_sota_labels(bag, &roster);
    let lanes: Vec<harness::book::Route> = harness::lane_route().into_iter().collect();
    let mut system = harness::book::y_types::cockpit_entry(
        workspace,
        &harness::delegate_roles(&specialists),
        &capabilities_block(&withheld),
        &lanes,
    );
    if let Some(sign) = crate::agent::tools::vision::vision_sidecar_prompt_hint_in(
        &crate::agent::tools::vision::VisionBackend::new(bag.codex_startup.as_deref()),
        bag.in_hand().as_ref(),
    ) {
        system.push('\n');
        system.push_str(&sign);
    }
    system
}

pub(crate) const WORKSPACE_CONTEXT_HEADER: &str = "[workspace-context/v1]";
pub(crate) fn workspace_context_message(
    workspace: &Path,
    project_guidance: String,
    skills_catalog: String,
    evidence: String,
) -> Option<ChatMsg> {
    workspace_context_message_with_live(workspace, project_guidance, skills_catalog, evidence, None)
}

pub(crate) fn workspace_context_message_with_live(
    workspace: &Path,
    project_guidance: String,
    skills_catalog: String,
    evidence: String,
    live_dir: Option<&Path>,
) -> Option<ChatMsg> {
    let mut payload = serde_json::json!({
        "workspace": workspace.to_string_lossy(),
        "project_guidance": project_guidance,
        "skills_catalog": skills_catalog,
        "evidence": evidence,
    });
    if let Some(live_dir) = live_dir {
        payload["live_dir"] = live_dir.to_string_lossy().into();
    }
    Some(ChatMsg::harness(format!(
        "{WORKSPACE_CONTEXT_HEADER}\n{payload}"
    )))
}

pub(crate) fn is_workspace_context(message: &ChatMsg) -> bool {
    message.role == ChatRole::Harness && message.content.starts_with(WORKSPACE_CONTEXT_HEADER)
}

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
    history.extend(build_workspace_context(workspace, skills));
    history
}

fn build_workspace_context(workspace: &Path, skills: &[harness::Skill]) -> Option<ChatMsg> {
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
    let live_dir = crate::agent::hotload::enabled()
        .then(crate::agent::hotload::root)
        .filter(|root| root.is_dir());
    workspace_context_message_with_live(
        workspace,
        harness::project_context(workspace),
        harness::skills_catalog(skills),
        evidence,
        live_dir.as_deref(),
    )
}

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
    let map = harness::task_system_contract(workspace);
    if !map.trim().is_empty() {
        history.push(ChatMsg::system(map.trim_start().to_string()));
    }
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

    #[test]
    fn the_workspace_map_rides_after_the_standing_text() {
        let dir = |name: &str| {
            let root = std::env::temp_dir().join(format!("angel-map-order-{}-{name}", std::process::id()));
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join("lib.rs"), "").unwrap();
            root
        };
        let (one, two) = (dir("one"), dir("two"));
        let _guard = crate::tests::env_lock();
        let _pace = crate::tests::TestEnvGuard::set("ANGEL_TASK_PACE_RESOLVED", "rapid");
        let _map = crate::tests::TestEnvGuard::set("ANGEL_TASK_WORKSPACE_MAP", "1");
        let history = |root: &Path| build_task_history(&[], &[], root, None);
        let (first, second) = (history(&one), history(&two));
        assert_eq!(first[0].content, second[0].content);
        assert!(!first[0].content.contains(&*one.to_string_lossy()));
        assert_eq!(first[1].role, ChatRole::System);
        assert!(first[1].content.contains("lib.rs"), "{}", first[1].content);
        let canonical = one.canonicalize().unwrap();
        assert!(first[1].content.contains(&*canonical.to_string_lossy()));
        for root in [one, two] {
            std::fs::remove_dir_all(root).unwrap();
        }
    }

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

pub(crate) fn build_registry(bag: &Bag, session_id: &str, workspace: PathBuf) -> ToolRegistry {
    let skills = harness::load_skills_for(&workspace);
    build_registry_with_skills(bag, session_id, workspace, skills)
}

pub(crate) fn build_history_and_registry(
    bag: &Bag,
    session_id: &str,
    workspace: PathBuf,
) -> (Vec<ChatMsg>, ToolRegistry) {
    let skills = harness::load_skills_for(&workspace);
    let mut history = vec![ChatMsg::system(build_system_prompt(bag, &workspace))];
    let (context, registry) = std::thread::scope(|scope| {
        let context = std::thread::Builder::new()
            .name("angel-workspace-context".into())
            .spawn_scoped(scope, || build_workspace_context(&workspace, &skills));
        let registry =
            build_registry_with_skills(bag, session_id, workspace.clone(), skills.clone());
        let context = match context {
            Ok(worker) => worker
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
            Err(_) => build_workspace_context(&workspace, &skills),
        };
        (context, registry)
    });
    history.extend(context);
    (history, registry)
}

pub(crate) fn warm_startup(workspace: &Path) {
    crate::agent::tools::self_model::prefetch_context(workspace);
    crate::agent::tools::build::prefetch_verifier_runtimes(workspace);
    let roots = [
        Some(workspace.to_path_buf()),
        crate::agent::tools::self_model::source_root(),
        Some(crate::platform::runtime_paths::cockpit_dir().join("skills")),
    ];
    for root in roots.into_iter().flatten().filter(|root| root.is_dir()) {
        let _ = std::thread::Builder::new()
            .name("angel-identity-warm".into())
            .spawn(move || {
                let _ = crate::platform::workspace_store::repo_identity(&root);
            });
    }
    let _ = std::thread::Builder::new()
        .name("angel-tls-warm".into())
        .spawn(|| {
            drop(ureq::AgentBuilder::new());
        });
}

fn build_registry_with_skills(
    bag: &Bag,
    session_id: &str,
    workspace: PathBuf,
    skills: Vec<harness::Skill>,
) -> ToolRegistry {
    let roster = delegation_roster(bag);
    let mut registry = ToolRegistry::with_team_self_in_run(
        workspace.clone(),
        roster,
        Some(bag.in_hand()),
        bag.codex_startup.as_deref(),
    );
    registry.set_backplane(crate::agent::backplane::BackplaneRegistry::from_bag(bag));
    registry.set_skill_index(&skills);
    if !skills.is_empty() {
        registry.register(Box::new(harness::SkillTool::for_workspace(
            skills,
            workspace.clone(),
        )));
    }
    registry.register(Box::new(
        crate::agent::tools::self_model::SelfMapTool::for_workspace(&workspace),
    ));
    registry.register(Box::new(
        crate::agent::tools::work_landing::WorkLandingTool::new(workspace.clone()),
    ));
    install_mcp_tools(&mut registry, &workspace);
    install_lsp_tools(&mut registry, &workspace);
    let store = crate::knowledge::memory::store::connect_from_env();
    if store.is_live() {
        registry.register(Box::new(harness::RecallTool::new(
            Arc::clone(&store),
            &workspace,
        )));
    }
    registry.enable_tool_search();
    registry.set_memory_store(store);
    registry.set_session_id(session_id);
    registry.set_aux_clubs(utility_roster(bag));
    registry
}

pub(crate) fn utility_roster(bag: &Bag) -> Vec<Arc<dyn Club>> {
    bag.roster()
        .into_iter()
        .filter(|c| !is_sota_label(c.label()) && c.label() != "practice" && !c.reports_to_palace())
        .collect()
}

fn withheld_sota_labels(bag: &Bag, roster: &[Arc<dyn Club>]) -> Vec<String> {
    let offered: Vec<&str> = roster.iter().map(|club| club.label()).collect();
    bag.roster()
        .iter()
        .map(|club| club.label().to_string())
        .filter(|label| !offered.contains(&label.as_str()) && label != bag.in_hand_label())
        .collect()
}

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
    for composition in compositions {
        let provider = composition.server.clone();
        let (tools, teardown) = composition.into_parts();
        if tools.is_empty() {
            teardown();
            continue;
        }
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
