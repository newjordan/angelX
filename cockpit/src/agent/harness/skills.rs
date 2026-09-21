//! Skills: discovery, catalog, and the `skill` tool.

use super::*;

pub(crate) const MAX_SKILL_FILE_BYTES: usize = 256 * 1024;
pub(crate) const MAX_SKILL_BODY_BYTES: usize = 64 * 1024;
const MAX_PLUGIN_METADATA_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_SKILLS_PER_DIR: usize = 256;

/// Public-safe shipped playbooks are compile-time assets, not an incidental dependency on
/// the source checkout that built the executable. Public installs contain the
/// binary without `CARGO_MANIFEST_DIR`; keeping the canonical files embedded
/// prevents a relocated install from silently losing its entire skill catalog.
/// A readable filesystem bundle is still scanned later as an overlay, so source
/// builds retain resource discovery and operators can replace a shipped body.
pub(crate) const EMBEDDED_SKILL_SOURCES: &[(&str, &str)] = &[
    (
        "benchmark-inference",
        include_str!("../../../skills/benchmark-inference/SKILL.md"),
    ),
    (
        "competition-loop",
        include_str!("../../../skills/competition-loop/SKILL.md"),
    ),
    (
        "gpu-fleet-recon",
        include_str!("../../../skills/gpu-fleet-recon/SKILL.md"),
    ),
    (
        "hf-model-ops",
        include_str!("../../../skills/hf-model-ops/SKILL.md"),
    ),
    (
        "navigate-code",
        include_str!("../../../skills/navigate-code/SKILL.md"),
    ),
    (
        "research-answer",
        include_str!("../../../skills/research-answer/SKILL.md"),
    ),
    ("plan", include_str!("../../../skills/plan/SKILL.md")),
    (
        "quantize-model",
        include_str!("../../../skills/quantize-model/SKILL.md"),
    ),
    (
        "requesting-code-review",
        include_str!("../../../skills/requesting-code-review/SKILL.md"),
    ),
    (
        "self-modify",
        include_str!("../../../skills/self-modify/SKILL.md"),
    ),
    (
        "serve-local-llm",
        include_str!("../../../skills/serve-local-llm/SKILL.md"),
    ),
    (
        "simplify-code",
        include_str!("../../../skills/simplify-code/SKILL.md"),
    ),
    (
        "systematic-debugging",
        include_str!("../../../skills/systematic-debugging/SKILL.md"),
    ),
    (
        "test-driven-development",
        include_str!("../../../skills/test-driven-development/SKILL.md"),
    ),
    (
        "verify-changes",
        include_str!("../../../skills/verify-changes/SKILL.md"),
    ),
    (
        "write-skill",
        include_str!("../../../skills/write-skill/SKILL.md"),
    ),
];

pub(crate) fn embedded_skills() -> Vec<Skill> {
    EMBEDDED_SKILL_SOURCES
        .iter()
        .filter_map(|(fallback, text)| {
            let skill = parse_skill(text, fallback);
            safe_skill_name(&skill.name).then_some(skill)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Skills — focused, on-demand playbooks (the Codex/Claude SKILL.md model). The
// catalog (name + one-line description) rides in the system prompt; the full body
// is fetched only when the agent calls `skill(name)`, so idle skills cost almost
// nothing in context.
// ---------------------------------------------------------------------------

/// One discovered skill loaded from a SKILL.md (or flat `*.md`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,
    /// User-authored skills are global by default. Machine-derived project
    /// skills must carry both fields and are inert outside that exact canonical
    /// repository. A legacy Habitsmith skill without them fails closed.
    pub(crate) project_scoped: bool,
    pub(crate) project_key: Option<String>,
    pub(crate) project_root: Option<PathBuf>,
    /// OpenCode-compatible manual-only discovery. The exact skill remains
    /// callable, but automatic routing and model-facing catalogs omit it.
    pub(crate) manual_only: bool,
    /// Admitted directory-style skill root. Supporting files are enumerated
    /// only after an exact load and never read into the skill body.
    pub(crate) base_dir: Option<PathBuf>,
}

impl Skill {
    fn applies_to(&self, workspace: &Path) -> bool {
        if !self.project_scoped {
            return true;
        }
        let identity = crate::platform::workspace_store::repo_identity(workspace);
        self.project_key.as_deref() == Some(identity.key.as_str())
            && self.project_root.as_deref() == Some(identity.root.as_path())
    }
}

/// Bounded routing metadata retained by the registry. Full skill bodies remain
/// solely behind `skill(name)` so task routing does not enlarge the fixed prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SkillSummary {
    name: String,
    description: String,
}

pub(crate) const SKILL_HINT_HEADER: &str = "[skill hint — harness-selected playbook]";
pub(crate) const SKILL_HINT_SENTINEL: &str = "[/skill hint]";
pub(crate) const MAX_SKILL_AUDIT_ISSUES: usize = 24;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SkillCatalogAudit {
    pub(crate) configured_sources: usize,
    pub(crate) available_sources: usize,
    pub(crate) usable_entries: usize,
    pub(crate) active_skills: usize,
    pub(crate) manual_only_skills: usize,
    pub(crate) rejected_entries: usize,
    pub(crate) capped_entries: usize,
    pub(crate) issues: Vec<SkillAuditIssue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SkillAuditIssue {
    pub(crate) source: PathBuf,
    pub(crate) entry: String,
    pub(crate) reason: &'static str,
}

#[derive(Clone, Default)]
struct SkillScan {
    available: bool,
    skills: Vec<Skill>,
    rejected_entries: usize,
    capped_entries: usize,
    issues: Vec<(String, &'static str)>,
}

pub(crate) fn skill_summaries(skills: &[Skill]) -> Vec<SkillSummary> {
    skills
        .iter()
        .filter(|skill| !skill.manual_only)
        .map(|skill| SkillSummary {
            name: skill.name.clone(),
            description: skill.description.clone(),
        })
        .collect()
}

fn canonical_skill_token(raw: &str) -> String {
    let token = raw
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_ascii_lowercase();
    match token.as_str() {
        "bug" | "bugs" | "broken" | "crash" | "crashes" | "crashing" | "crashed" | "error"
        | "errors" | "fail" | "fails" | "failed" | "failing" | "failure" | "failures"
        | "debugger" | "debugging" => "debug".to_string(),
        "tests" | "tested" | "testing" | "tdd" => "test".to_string(),
        "reviews" | "reviewing" | "audit" | "auditing" => "review".to_string(),
        "verification" | "verified" | "verifying" | "validate" | "validated" | "validating"
        | "proof" | "prove" => "verify".to_string(),
        "navigation" | "navigating" | "explore" | "exploring" => "navigate".to_string(),
        "refactor" | "refactors" | "refactoring" | "cleanup" | "dedupe" => "simplify".to_string(),
        _ => token,
    }
}

/// Description tokens that are too generic to corroborate a weak name hit.
/// Without this, ambient "the tests all pass" matched TDD via desc "pass".
fn is_weak_corroboration_token(token: &str) -> bool {
    matches!(
        token,
        "already"
            | "done"
            | "first"
            | "good"
            | "green"
            | "loop"
            | "make"
            | "next"
            | "new"
            | "old"
            | "pass"
            | "passed"
            | "passing"
            | "red"
            | "then"
            | "well"
    )
}

fn is_skill_stopword(token: &str) -> bool {
    matches!(
        token,
        // Generic glue tokens + common English function words. A single hit of
        // these (e.g. "model" in `hf-model-ops`, or "the" shared by ambient
        // prose and a description) used to fire a wrong automatic playbook;
        // prefer no hint over noise.
        "about"
            | "after"
            | "agent"
            | "all"
            | "also"
            | "and"
            | "any"
            | "are"
            | "before"
            | "but"
            | "can"
            | "change"
            | "changes"
            | "code"
            | "coding"
            | "could"
            | "data"
            | "development"
            | "does"
            | "driven"
            | "each"
            | "file"
            | "files"
            | "for"
            | "from"
            | "had"
            | "has"
            | "have"
            | "harness"
            | "how"
            | "into"
            | "its"
            | "just"
            | "may"
            | "model"
            | "models"
            | "more"
            | "most"
            | "not"
            | "only"
            | "ops"
            | "our"
            | "out"
            | "over"
            | "please"
            | "requesting"
            | "should"
            | "skill"
            | "some"
            | "such"
            | "system"
            | "systematic"
            | "task"
            | "than"
            | "that"
            | "the"
            | "their"
            | "them"
            | "then"
            | "there"
            | "these"
            | "they"
            | "this"
            | "those"
            | "through"
            | "tool"
            | "tools"
            | "under"
            | "very"
            | "was"
            | "were"
            | "what"
            | "when"
            | "who"
            | "why"
            | "will"
            | "with"
            | "work"
            | "working"
            | "would"
            | "you"
            | "your"
    )
}

fn skill_token_list(text: &str) -> Vec<String> {
    skill_token_pairs(text)
        .into_iter()
        .map(|(_, canon)| canon)
        .collect()
}

/// `(raw_lower, canonical)` pairs after stopword filter on the canonical form.
fn skill_token_pairs(text: &str) -> Vec<(String, String)> {
    text.chars()
        .take(4096)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .filter_map(|raw| {
            let raw = raw
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_ascii_lowercase();
            if raw.len() < 3 {
                return None;
            }
            let canon = canonical_skill_token(&raw);
            if canon.len() < 3 || is_skill_stopword(&canon) {
                return None;
            }
            Some((raw, canon))
        })
        .collect()
}

fn skill_tokens(text: &str) -> std::collections::BTreeSet<String> {
    skill_token_list(text).into_iter().collect()
}

/// Rough English stem so tests/testing/tested collapse, but bug/crash stay distinct.
fn rough_stem(token: &str) -> String {
    let mut t = token.to_ascii_lowercase();
    for suf in ["ing", "ed", "es", "s"] {
        if t.len() > suf.len() + 2 && t.ends_with(suf) {
            t.truncate(t.len() - suf.len());
            break;
        }
    }
    t
}

/// Ambient English often stacks fail/error without meaning "debug this".
/// Those surfaces share the `debug` canon but must not *pair* to clear the
/// name-hit gate the way bug+crash does (A5).
///
/// Also weak: "broken" (broken error path / broken link) and debugger/debugging
/// gerunds that co-occur with fail/error in ordinary prose without a real
/// incident (bug/crash remain strong).
fn is_weak_debug_surface(raw: &str) -> bool {
    matches!(
        raw,
        "fail"
            | "fails"
            | "failed"
            | "failing"
            | "failure"
            | "failures"
            | "error"
            | "errors"
            | "broken"
            | "debugger"
            | "debugging"
    )
}

/// Ambient process language ("proof and validate", "verification complete")
/// shares the `verify` canon — collapse so it cannot alone fire verify-changes.
fn is_weak_verify_surface(raw: &str) -> bool {
    matches!(
        raw,
        "verification"
            | "verified"
            | "verifying"
            | "validate"
            | "validated"
            | "validating"
            | "proof"
            | "prove"
            | "verify"
    )
}

/// Ambient "audit the reviews" / "reviewing and auditing" is not a playbook ask.
fn is_weak_review_surface(raw: &str) -> bool {
    matches!(
        raw,
        "review" | "reviews" | "reviewing" | "audit" | "auditing"
    )
}

/// Ambient "cleanup the refactor" is not simplify-code without stronger intent.
fn is_weak_simplify_surface(raw: &str) -> bool {
    matches!(
        raw,
        "refactor" | "refactors" | "refactoring" | "cleanup" | "dedupe"
    )
}

/// Ambient "explore the navigation" / "navigating and exploring" wiki prose.
fn is_weak_navigate_surface(raw: &str) -> bool {
    matches!(
        raw,
        "navigation" | "navigating" | "explore" | "exploring" | "navigate"
    )
}

/// Stem key used for distinct-name evidence. Weak synonym surfaces of a canon
/// collapse to one bucket so ambient synonym pairs cannot alone clear the
/// name-hit gate (A5). Strong debug evidence (bug/crash) stays separate.
fn evidence_stem(raw: &str, canon: &str) -> String {
    if canon == "debug" && is_weak_debug_surface(raw) {
        return "weak_debug".to_string();
    }
    if canon == "verify" && is_weak_verify_surface(raw) {
        return "weak_verify".to_string();
    }
    if canon == "review" && is_weak_review_surface(raw) {
        return "weak_review".to_string();
    }
    if canon == "simplify" && is_weak_simplify_surface(raw) {
        return "weak_simplify".to_string();
    }
    if canon == "navigate" && is_weak_navigate_surface(raw) {
        return "weak_navigate".to_string();
    }
    rough_stem(raw)
}

/// Distinct name evidence for the weak-hit gate. Within each canonical name
/// token, morphological variants (tests/testing/retesting) collapse via rough
/// stem + prefix merge; strong synonyms that share a canon but not a stem
/// (bug/crash → debug) remain separate and can still clear the gate. Weak
/// fail/error debug surfaces collapse together (A5).
fn distinct_name_evidence(
    pairs: &[(String, String)],
    name_tokens: &std::collections::BTreeSet<String>,
) -> usize {
    let mut by_canon: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        std::collections::BTreeMap::new();
    for (raw, canon) in pairs {
        if name_tokens.contains(canon) {
            by_canon
                .entry(canon.clone())
                .or_default()
                .insert(evidence_stem(raw, canon));
        }
    }
    by_canon.values().map(merge_prefix_stems).sum()
}

fn merge_prefix_stems(stems: &std::collections::BTreeSet<String>) -> usize {
    let mut ordered: Vec<&String> = stems.iter().collect();
    ordered.sort_by_key(|s| s.len());
    let mut roots: Vec<&str> = Vec::new();
    for s in ordered {
        if roots
            .iter()
            .any(|root| s.starts_with(root) || root.starts_with(s.as_str()))
        {
            continue;
        }
        roots.push(s.as_str());
    }
    roots.len()
}

pub(crate) fn safe_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
}

/// Explicit multi-segment slug mention (`systematic-debugging`, `write_skill`).
/// Single-token names are never "explicit" via substring — ambient English
/// contains `plan`, `write`, `review`, `loop`, etc.
///
/// A5: require segment boundaries so `write-skillfully` / `precompetition-loopback`
/// / `unsystematic-debugging-helper` do not count as naming the playbook.
fn explicit_skill_slug_in_task(task_lc: &str, name_slug: &str) -> bool {
    let multi = name_slug.contains('-')
        || name_slug.contains('_')
        || name_slug.contains('.')
        || name_slug.contains(':');
    if !multi || name_slug.is_empty() {
        return false;
    }
    slug_mentioned_as_segment(task_lc, name_slug)
}

/// True when `slug` appears in `hay` with non-alphanumeric (or string edge)
/// neighbors — a whole skill-name token, not a longer compound word.
fn slug_mentioned_as_segment(hay: &str, slug: &str) -> bool {
    let hay_b = hay.as_bytes();
    let slug_b = slug.as_bytes();
    if slug_b.is_empty() || hay_b.len() < slug_b.len() {
        return false;
    }
    let mut start = 0;
    while start + slug_b.len() <= hay_b.len() {
        if let Some(rel) = hay[start..].find(slug) {
            let i = start + rel;
            let before_ok = i == 0 || !hay_b[i - 1].is_ascii_alphanumeric();
            let after = i + slug_b.len();
            let after_ok = after >= hay_b.len() || !hay_b[after].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
            start = i + 1;
        } else {
            break;
        }
    }
    false
}

/// Select at most one high-confidence playbook using only deterministic lexical
/// evidence. A skill must match its *name* (description-only overlap is never
/// enough), while description overlap merely breaks ties. The returned hint is
/// bounded and contains no repository-controlled description/body text.
///
/// Policy: **prefer no hint over a wrong one.** Weak single-token name hits
/// (the `hf-model-ops` vs ambient "model" failure, and ambient `plan` / `write`
/// / `review` / `loop` false positives) are suppressed unless corroborated.
pub(crate) fn relevant_skill_hint(skills: &[SkillSummary], task: &str) -> Option<String> {
    let name = relevant_skill_name(skills, task)?;
    Some(format!(
        "{SKILL_HINT_HEADER}\nRelevant playbook: `{name}`. Call `skill(name=\"{name}\")` before acting if it fits this task.\n{SKILL_HINT_SENTINEL}\n\n"
    ))
}

// This playbook changes completion semantics, so incidental words in a long
// implementation contract cannot select it. Explicit skill names still win.
fn research_answer_intent(task: &str) -> bool {
    let request = task
        .trim_start()
        .split("\n\n")
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    [
        "research question",
        "answer this question",
        "answer the question",
        "answer using sources",
        "answer with citations",
    ]
    .iter()
    .any(|phrase| request.contains(phrase))
}

pub(crate) fn relevant_skill_name<'a>(skills: &'a [SkillSummary], task: &str) -> Option<&'a str> {
    if task.trim().is_empty() {
        return None;
    }
    let query_pairs = skill_token_pairs(task);
    if query_pairs.is_empty() {
        return None;
    }
    let query: std::collections::BTreeSet<String> =
        query_pairs.iter().map(|(_, c)| c.clone()).collect();
    let task_lc = task.to_ascii_lowercase();
    let mut ranked = skills
        .iter()
        .filter(|skill| safe_skill_name(&skill.name))
        .filter_map(|skill| {
            let name_tokens = skill_tokens(&skill.name);
            // Names that only contain stopwords after filtering (e.g. `hf-model-ops`
            // → empty) can never fire a hint — prefer silence over a bad match.
            if name_tokens.is_empty() {
                return None;
            }
            let name_matches = query.intersection(&name_tokens).count();
            if name_matches == 0 {
                return None;
            }
            // Distinct stem evidence for name hits (A5): tests+testing → 1,
            // bug+crash → 2. Morphological doubles must not clear the gate alone.
            let name_hit_weight = distinct_name_evidence(&query_pairs, &name_tokens);
            let name_slug = skill.name.to_ascii_lowercase();
            let name_in_task = explicit_skill_slug_in_task(&task_lc, &name_slug);
            // This playbook changes the turn into an answer-only contract.
            // A coding task can mention research, sources and proofs without
            // requesting an answer-only workflow (observed in live MODEXP).
            // Description overlap must not promote one ambient name token.
            if name_slug == "research-answer" && !name_in_task && !research_answer_intent(task) {
                return None;
            }
            let description_tokens = skill_tokens(&skill.description);
            // Description corroboration must add tokens *beyond* the name hit set
            // so "plan" in both name and description cannot self-confirm, and must
            // not be a weak generic (pass/make/first) that ambient English shares.
            let description_corroboration = query
                .intersection(&description_tokens)
                .filter(|token| {
                    !name_tokens.contains(token.as_str()) && !is_weak_corroboration_token(token)
                })
                .count()
                .min(8);
            let description_matches = query.intersection(&description_tokens).count().min(8);
            // Prefer silence: need two distinct name-stem hits, a multi-segment
            // slug mention, or a description token that is not already the name hit.
            if name_hit_weight < 2 && description_corroboration == 0 && !name_in_task {
                return None;
            }
            let score = name_matches * 8
                + description_matches
                + name_hit_weight.saturating_sub(1) * 2
                + if name_in_task { 4 } else { 0 };
            Some((score, name_matches, &skill.name))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.2.cmp(right.2))
    });
    let (score, _name_hits, name) = ranked.first()?;
    if *score < 8 {
        return None;
    }
    // Ambiguous top-2: no hint beats a coin-flip playbook.
    if ranked.len() >= 2 {
        let second = ranked[1].0;
        if *score < second.saturating_add(4) {
            return None;
        }
    }
    Some(name.as_str())
}

/// Discover skills from the compatible harness surfaces available to
/// `workspace`: bundled angelX skills, repo `.claude/skills`,
/// `.opencode/skills`, and `.agents/skills`, plugin-bundled skills, compatible
/// global skill roots, and angelX user skills.
///
/// Because angelX's `skill(name)` tool is name-addressed, duplicate names are
/// merged with later sources winning. Deeper repo scopes beat shallower ones;
/// within one scope `.agents` beats `.opencode`, which beats `.claude`.
/// `~/.angelX/skills` remains the final explicit user override.
pub fn load_skills_for(workspace: &Path) -> Vec<Skill> {
    let scoped = |dir: &Path| {
        cached_skill_scan(dir)
            .skills
            .into_iter()
            .filter(|skill| skill.applies_to(workspace))
            .collect::<Vec<_>>()
    };
    let mut skills = embedded_skills();
    for dir in skill_source_dirs(workspace) {
        skills = merge_skills(skills, scoped(&dir));
    }
    skills
}

fn configured_user_skill_dir() -> PathBuf {
    std::env::var_os("ANGEL_SKILLS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".angelX/skills")
        })
}

fn skill_source_dirs(workspace: &Path) -> Vec<PathBuf> {
    // Skills angelX ships with — methodology playbooks adapted from the Hermes
    // library. `ANGEL_BUNDLED_SKILLS_DIR` overrides the crate-local default.
    let bundled = std::env::var_os("ANGEL_BUNDLED_SKILLS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::platform::runtime_paths::cockpit_dir().join("skills"));
    let mut dirs = vec![bundled];
    dirs.extend(repo_skill_dirs(workspace));
    dirs.extend(plugin_skill_dirs(workspace));
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.extend([
            home.join(".claude/skills"),
            home.join(".config/opencode/skills"),
            home.join(".agents/skills"),
        ]);
    }
    dirs.push(configured_user_skill_dir());
    dirs
}

pub(crate) fn audit_skills_for(workspace: &Path) -> SkillCatalogAudit {
    let sources = skill_source_dirs(workspace);
    let embedded = embedded_skills();
    let mut audit = SkillCatalogAudit {
        configured_sources: sources.len().saturating_add(1),
        available_sources: 1,
        usable_entries: embedded.len(),
        ..SkillCatalogAudit::default()
    };
    let mut active = embedded;
    for source in sources {
        let scan = cached_skill_scan(&source);
        audit.available_sources += usize::from(scan.available);
        audit.usable_entries += scan.skills.len();
        audit.rejected_entries += scan.rejected_entries;
        audit.capped_entries += scan.capped_entries;
        let applicable = scan
            .skills
            .iter()
            .filter(|skill| skill.applies_to(workspace))
            .cloned()
            .collect();
        active = merge_skills(active, applicable);
        for (entry, reason) in scan.issues {
            if audit.issues.len() >= MAX_SKILL_AUDIT_ISSUES {
                break;
            }
            audit.issues.push(SkillAuditIssue {
                source: source.clone(),
                entry,
                reason,
            });
        }
    }
    audit.active_skills = active.len();
    audit.manual_only_skills = active.iter().filter(|skill| skill.manual_only).count();
    audit
}

pub(crate) fn repo_skill_dirs(workspace: &Path) -> Vec<PathBuf> {
    let start = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let mut git_root: Option<PathBuf> = None;
    let mut probe: Option<&Path> = Some(start.as_path());
    while let Some(dir) = probe {
        if is_git_root(dir) {
            git_root = Some(dir.to_path_buf());
            break;
        }
        probe = dir.parent();
    }
    let (chain, confinement_root): (Vec<PathBuf>, PathBuf) = match git_root {
        Some(root) => {
            let mut v = Vec::new();
            let mut cur = start.as_path();
            loop {
                v.push(cur.to_path_buf());
                if cur == root {
                    break;
                }
                match cur.parent() {
                    Some(p) => cur = p,
                    None => break,
                }
            }
            v.reverse();
            (v, root)
        }
        None => (vec![start.clone()], start),
    };
    chain
        .into_iter()
        .flat_map(|dir| {
            [
                dir.join(".claude/skills"),
                dir.join(".opencode/skills"),
                dir.join(".agents/skills"),
            ]
        })
        .filter(|dir| {
            dir.canonicalize()
                .is_ok_and(|resolved| resolved.starts_with(&confinement_root))
        })
        .collect()
}

pub(crate) fn plugin_skill_dirs(workspace: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(root) = git_root_for(workspace) {
        let marketplace = root.join(".agents/plugins/marketplace.json");
        if marketplace
            .canonicalize()
            .is_ok_and(|resolved| resolved.starts_with(&root))
        {
            out.extend(
                plugin_skill_dirs_from_marketplace(&marketplace)
                    .into_iter()
                    .filter(|dir| {
                        dir.canonicalize()
                            .is_ok_and(|resolved| resolved.starts_with(&root))
                    }),
            );
        }
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        out.extend(plugin_skill_dirs_from_marketplace(
            &home.join(".agents/plugins/marketplace.json"),
        ));
    }
    out
}

pub(crate) fn git_root_for(start: &Path) -> Option<PathBuf> {
    let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    let mut probe: Option<&Path> = Some(start.as_path());
    while let Some(dir) = probe {
        if is_git_root(dir) {
            return Some(dir.to_path_buf());
        }
        probe = dir.parent();
    }
    None
}

pub(crate) fn plugin_skill_dirs_from_marketplace(path: &Path) -> Vec<PathBuf> {
    let Some(text) = read_bounded_utf8_file(path, MAX_PLUGIN_METADATA_BYTES) else {
        return Vec::new();
    };
    let v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let root = marketplace_root(path);
    let Some(plugins) = v.get("plugins").and_then(|p| p.as_array()) else {
        return Vec::new();
    };
    plugins
        .iter()
        .filter_map(|entry| {
            let rel = entry.pointer("/source/path").and_then(|p| p.as_str())?;
            let plugin_dir = expand_marketplace_path(&root, rel);
            plugin_skill_dir(&plugin_dir)
        })
        .collect()
}

pub(crate) fn marketplace_root(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if parent.file_name().and_then(|s| s.to_str()) == Some("plugins")
        && parent
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            == Some(".agents")
    {
        return parent
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(parent)
            .to_path_buf();
    }
    parent.to_path_buf()
}

pub(crate) fn expand_marketplace_path(root: &Path, raw: &str) -> PathBuf {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME").map(PathBuf::from)
    {
        return home.join(rest);
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

pub(crate) fn plugin_skill_dir(plugin_dir: &Path) -> Option<PathBuf> {
    let manifest = plugin_dir.join(".codex-plugin/plugin.json");
    let text = read_bounded_utf8_file(&manifest, MAX_PLUGIN_METADATA_BYTES)?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let skills = v
        .get("skills")
        .and_then(|s| s.as_str())
        .unwrap_or("./skills/");
    Some(expand_marketplace_path(plugin_dir, skills))
}

fn read_bounded_utf8_file(path: &Path, max_bytes: usize) -> Option<String> {
    let parent = path.parent()?;
    let name = Path::new(path.file_name()?);
    let bytes = confined_read_limited(parent, name, max_bytes).ok()??;
    String::from_utf8(bytes).ok()
}

/// Merge bundled + user skills, **user wins** on name collision. Sorted by name
/// (via `BTreeMap`) for a stable catalog. Pure → testable.
pub(crate) fn merge_skills(bundled: Vec<Skill>, user: Vec<Skill>) -> Vec<Skill> {
    let mut by_name: std::collections::BTreeMap<String, Skill> = std::collections::BTreeMap::new();
    for s in bundled {
        by_name.insert(s.name.clone(), s);
    }
    for s in user {
        by_name.insert(s.name.clone(), s); // user overrides bundled
    }
    by_name.into_values().collect()
}

/// Discovery, split out from the env lookup so it's testable. Two layouts are
/// accepted: a folder per skill (`<dir>/<name>/SKILL.md`, the Codex/Claude
/// convention) and a flat file (`<dir>/<name>.md`). Unreadable entries are
/// skipped, never fatal. Result is sorted by name for a stable catalog.
#[cfg(test)]
pub(crate) fn load_skills_from(dir: &Path) -> Vec<Skill> {
    scan_skills_from(dir).skills
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SkillSourceSignature {
    entry_count: usize,
    entries: Vec<SkillEntrySignature>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SkillEntrySignature {
    name: std::ffi::OsString,
    is_dir: bool,
    file: Option<SkillFileSignature>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SkillFileSignature {
    len: u64,
    modified_nanos: Option<u128>,
    readonly: bool,
    is_file: bool,
    is_symlink: bool,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    change_secs: i64,
    #[cfg(unix)]
    change_nanos: i64,
}

#[derive(Clone)]
struct CachedSkillScan {
    signature: SkillSourceSignature,
    scan: SkillScan,
    #[cfg(test)]
    hits: usize,
}

const MAX_SKILL_SCAN_CACHE_SOURCES: usize = 64;

fn skill_scan_cache()
-> &'static std::sync::Mutex<std::collections::HashMap<PathBuf, CachedSkillScan>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<PathBuf, CachedSkillScan>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Cheaply fingerprint the exact sorted entry prefix the bounded loader can
/// admit. Directory entry names plus the candidate SKILL.md/file metadata make
/// ordinary catalog reads avoid reparsing every body, while edits, replacement,
/// creation, removal, or ordering changes invalidate the cache.
fn skill_source_signature(dir: &Path) -> Option<SkillSourceSignature> {
    let mut entries = confined_read_dir(dir, Path::new("")).ok()?;
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    let entry_count = entries.len();
    let entries = entries
        .into_iter()
        .take(MAX_SKILLS_PER_DIR)
        .map(|entry| {
            let path = PathBuf::from(&entry.name);
            let candidate = if entry.is_dir {
                Some(path.join("SKILL.md"))
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("md")
                && !path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| stem.eq_ignore_ascii_case("readme"))
            {
                Some(path)
            } else {
                None
            };
            SkillEntrySignature {
                name: entry.name,
                is_dir: entry.is_dir,
                file: candidate
                    .and_then(|path| std::fs::symlink_metadata(dir.join(path)).ok())
                    .map(|metadata| {
                        let modified_nanos = metadata
                            .modified()
                            .ok()
                            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|duration| duration.as_nanos());
                        #[cfg(unix)]
                        use std::os::unix::fs::MetadataExt;
                        SkillFileSignature {
                            len: metadata.len(),
                            modified_nanos,
                            readonly: metadata.permissions().readonly(),
                            is_file: metadata.file_type().is_file(),
                            is_symlink: metadata.file_type().is_symlink(),
                            #[cfg(unix)]
                            device: metadata.dev(),
                            #[cfg(unix)]
                            inode: metadata.ino(),
                            #[cfg(unix)]
                            change_secs: metadata.ctime(),
                            #[cfg(unix)]
                            change_nanos: metadata.ctime_nsec(),
                        }
                    }),
            }
        })
        .collect();
    Some(SkillSourceSignature {
        entry_count,
        entries,
    })
}

fn cached_skill_scan(dir: &Path) -> SkillScan {
    let cache_key = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let Some(before) = skill_source_signature(dir) else {
        return scan_skills_from(dir);
    };
    if let Ok(mut cache) = skill_scan_cache().lock()
        && let Some(cached) = cache.get_mut(&cache_key)
        && cached.signature == before
    {
        #[cfg(test)]
        {
            cached.hits = cached.hits.saturating_add(1);
        }
        return cached.scan.clone();
    }

    let scan = scan_skills_from(dir);
    // Never publish a snapshot assembled across a concurrent source mutation.
    let Some(after) = skill_source_signature(dir) else {
        return scan;
    };
    if before != after {
        return scan;
    }
    if let Ok(mut cache) = skill_scan_cache().lock() {
        if cache.len() >= MAX_SKILL_SCAN_CACHE_SOURCES && !cache.contains_key(&cache_key) {
            cache.clear();
        }
        cache.insert(
            cache_key,
            CachedSkillScan {
                signature: after,
                scan: scan.clone(),
                #[cfg(test)]
                hits: 0,
            },
        );
    }
    scan
}

#[cfg(test)]
pub(crate) fn skill_scan_cache_hits(dir: &Path) -> Option<usize> {
    let cache_key = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    skill_scan_cache()
        .lock()
        .ok()?
        .get(&cache_key)
        .map(|cached| cached.hits)
}

fn scan_skills_from(dir: &Path) -> SkillScan {
    let mut scan = SkillScan::default();
    let mut entries = match confined_read_dir(dir, Path::new("")) {
        Ok(entries) => entries,
        Err(_) => return scan, // no/unreadable skills dir → no skills
    };
    scan.available = true;
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    scan.capped_entries = entries.len().saturating_sub(MAX_SKILLS_PER_DIR);
    for entry in entries.into_iter().take(MAX_SKILLS_PER_DIR) {
        let path = PathBuf::from(&entry.name);
        let (md_path, fallback_name) = if entry.is_dir {
            (
                path.join("SKILL.md"),
                entry.name.to_string_lossy().to_string(),
            )
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            // A bare README.md isn't a skill; require a SKILL.md or a named *.md.
            if stem.eq_ignore_ascii_case("readme") {
                continue;
            }
            (path.clone(), stem.to_string())
        } else {
            continue;
        };
        let bytes = match confined_read_limited(dir, &md_path, MAX_SKILL_FILE_BYTES) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                scan.reject(&entry.name, "file exceeds 256 KiB");
                continue;
            }
            Err(_) => {
                scan.reject(&entry.name, "not a confined readable regular file");
                continue;
            }
        };
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                scan.reject(&entry.name, "not valid UTF-8");
                continue;
            }
        };
        let mut skill = parse_skill(&text, &fallback_name);
        if entry.is_dir {
            skill.base_dir = Some(dir.join(&path));
        }
        if safe_skill_name(&skill.name) {
            scan.skills.push(skill);
        } else {
            scan.reject(&entry.name, "unsafe or empty skill name");
        }
    }
    scan.skills.sort_by(|a, b| a.name.cmp(&b.name));
    scan
}

impl SkillScan {
    fn reject(&mut self, entry: &std::ffi::OsStr, reason: &'static str) {
        self.rejected_entries += 1;
        if self.issues.len() < MAX_SKILL_AUDIT_ISSUES {
            self.issues.push((safe_audit_entry_label(entry), reason));
        }
    }
}

pub(crate) fn safe_audit_entry_label(entry: &std::ffi::OsStr) -> String {
    let mut label = entry
        .to_string_lossy()
        .chars()
        .filter(|character| !character.is_control() && !unsafe_audit_format(*character))
        .take(80)
        .collect::<String>();
    if label.is_empty() {
        label.push_str("<unnamed>");
    }
    label
}

fn unsafe_audit_format(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

/// Parse a SKILL.md into name/description/body. `---`-delimited frontmatter at the
/// top supplies `name:` / `description:`; anything missing falls back to
/// `fallback_name` and the first non-blank, non-heading line of the body.
pub(crate) fn parse_skill(text: &str, fallback_name: &str) -> Skill {
    let mut name = fallback_name.to_string();
    let mut description = String::new();
    let mut body = text;
    let mut project_scoped = false;
    let mut project_key = None;
    let mut project_root = None;
    let mut machine_derived_project_skill = false;
    let mut manual_only = false;
    if let Some(rest) = text.strip_prefix("---")
        && let Some(end) = rest.find("\n---")
    {
        let front = &rest[..end];
        body = rest[end + 4..].trim_start_matches(['\r', '\n']);
        for line in front.lines() {
            if let Some(v) = line.strip_prefix("name:") {
                let v = v.trim().trim_matches('"').trim();
                if !v.is_empty() {
                    name = v.to_string();
                }
            } else if let Some(v) = line.strip_prefix("description:") {
                let v = v.trim().trim_matches('"').trim();
                if !v.is_empty() {
                    description = v.to_string();
                }
            } else if let Some(v) = line.strip_prefix("scope:") {
                project_scoped = v.trim().trim_matches('"') == "project";
            } else if let Some(v) = line.strip_prefix("repo_key:") {
                let v = v.trim().trim_matches('"').trim();
                if !v.is_empty() {
                    project_key = Some(v.to_string());
                }
            } else if let Some(v) = line.strip_prefix("repo_root:") {
                let v = v.trim().trim_matches('"').trim();
                if !v.is_empty() {
                    project_root = Some(PathBuf::from(v));
                }
            } else if let Some(v) = line.strip_prefix("fact:") {
                machine_derived_project_skill = v.trim().starts_with("hyp_habit_");
            }
            let line = line.trim();
            if let Some(v) = line
                .strip_prefix("opencode/autoinvoke:")
                .or_else(|| line.strip_prefix("metadata.opencode/autoinvoke:"))
            {
                manual_only = matches!(
                    v.trim().trim_matches('"').to_ascii_lowercase().as_str(),
                    "0" | "false" | "no" | "off"
                );
            }
        }
    }
    // Habitsmith has always emitted project-derived instructions. Older drafts
    // lacked an explicit binding; treating them as global is exactly the leak we
    // are preventing, so they become scoped-but-unbound and therefore inert.
    project_scoped |= machine_derived_project_skill;
    if description.is_empty() {
        description = body
            .lines()
            .map(|l| l.trim())
            .find(|l| !l.is_empty() && !l.starts_with('#'))
            .unwrap_or("")
            .chars()
            .take(160)
            .collect();
    }
    let mut body = body.trim().to_string();
    if body.len() > MAX_SKILL_BODY_BYTES {
        truncate_to_char_boundary(&mut body, MAX_SKILL_BODY_BYTES);
        body.push_str(
            "\n\n[skill instructions truncated by the harness at 65536 UTF-8 bytes; \
             inspect the source skill before treating this playbook as complete]",
        );
    }
    description = description.chars().take(160).collect();
    Skill {
        name,
        description,
        body,
        project_scoped,
        project_key,
        project_root,
        manual_only,
        base_dir: None,
    }
}

/// The compact skills catalog appended to the system prompt (empty when there are
/// none). Tells the model to call `skill(name)` to load full instructions.
pub fn skills_catalog(skills: &[Skill]) -> String {
    let visible = skills
        .iter()
        .filter(|skill| !skill.manual_only && safe_skill_name(&skill.name));
    if visible.clone().next().is_none() {
        return String::new();
    }
    let mut s = String::from(
        "\n\nSkills — focused playbooks you can load on demand. Call `skill(name)` to get \
         a skill's full instructions before doing that kind of task:\n",
    );
    // Names are constrained identifiers. Descriptions can originate in the
    // repository, user skill directories, or plugin metadata, so never splice
    // that free-form text into the System-role bootstrap prompt.
    for sk in visible {
        s.push_str(&format!("- `{}`\n", sk.name));
    }
    s
}

/// `skill(name)` — return a discovered skill's full instructions on demand.
/// Each load is recorded to the experience ledger (`event:"skill"`, H5 of the
/// Habitsmith plan) stamped with the workspace the catalog was built for, so
/// minted skills prove they're used and drift is detectable.
pub struct SkillTool {
    skills: Vec<Skill>,
    workspace: PathBuf,
}

impl SkillTool {
    #[cfg(test)]
    pub fn new(skills: Vec<Skill>) -> Self {
        Self::for_workspace(skills, current_dir_workspace())
    }

    pub fn for_workspace(skills: Vec<Skill>, workspace: PathBuf) -> Self {
        Self { skills, workspace }
    }
}

const MAX_SKILL_SUPPORT_PATHS: usize = 10;
const MAX_SKILL_SUPPORT_DIRS: usize = 16;
const MAX_SKILL_SUPPORT_ENTRIES_PER_DIR: usize = 256;

fn supporting_skill_paths(base: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut pending = std::collections::VecDeque::from([(PathBuf::new(), 0_usize)]);
    let mut visited_dirs = 0_usize;
    while let Some((relative_dir, depth)) = pending.pop_front() {
        if visited_dirs >= MAX_SKILL_SUPPORT_DIRS || paths.len() >= MAX_SKILL_SUPPORT_PATHS {
            break;
        }
        visited_dirs += 1;
        let Ok(mut entries) = confined_read_dir(base, &relative_dir) else {
            continue;
        };
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        for entry in entries.into_iter().take(MAX_SKILL_SUPPORT_ENTRIES_PER_DIR) {
            let Some(name) = entry.name.to_str() else {
                continue;
            };
            if name.chars().any(|character| character.is_control()) {
                continue;
            }
            if relative_dir.as_os_str().is_empty() && name == "SKILL.md" {
                continue;
            }
            let path = relative_dir.join(name);
            if entry.is_dir {
                if depth < 3 {
                    pending.push_back((path, depth + 1));
                }
            } else if confined_read_limited(base, &path, 0).is_ok() {
                paths.push(path);
                if paths.len() >= MAX_SKILL_SUPPORT_PATHS {
                    break;
                }
            }
        }
    }
    paths
}

fn loaded_skill_body(skill: &Skill) -> String {
    let Some(base) = skill.base_dir.as_deref() else {
        return skill.body.clone();
    };
    let paths = supporting_skill_paths(base);
    let mut output = skill.body.clone();
    output.push_str("\n\n[skill resources — contents not loaded]\nbase: ");
    output.push_str(&base.to_string_lossy());
    if paths.is_empty() {
        output.push_str("\nsupporting files: (none)");
    } else {
        output.push_str("\nsupporting files:");
        for path in paths {
            output.push_str("\n- ");
            output.push_str(&path.to_string_lossy());
        }
    }
    output
}

impl Tool for SkillTool {
    fn name(&self) -> &str {
        "skill"
    }
    fn def(&self) -> ToolDef {
        let names: Vec<&str> = self
            .skills
            .iter()
            .filter(|skill| !skill.manual_only)
            .map(|skill| skill.name.as_str())
            .collect();
        ToolDef {
            name: "skill".to_string(),
            description: format!(
                "Load a skill's full instructions by name before doing that kind of task. \
                 Available: {}.",
                if names.is_empty() {
                    "(none)".to_string()
                } else {
                    names.join(", ")
                }
            ),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "skill name from the catalog" }
                },
                "required": ["name"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let started = std::time::Instant::now();
        let name = args["name"].as_str().ok_or("missing 'name'")?;
        let found = self
            .skills
            .iter()
            .find(|s| s.name == name || s.name.eq_ignore_ascii_case(name));
        // Experience ledger: which skill loaded, where, and whether the lookup
        // resolved — Habitsmith's usage/drift signal. Best-effort; never
        // alters what the model gets back.
        crate::knowledge::experience::record_skill_event(
            &crate::knowledge::experience::SkillExperience {
                name,
                ok: found.is_some(),
                dur_ms: started.elapsed().as_millis(),
            },
            &self.workspace,
        );
        if let Some(sk) = found {
            return Ok(loaded_skill_body(sk));
        }
        let names: Vec<&str> = self
            .skills
            .iter()
            .filter(|skill| !skill.manual_only)
            .map(|skill| skill.name.as_str())
            .collect();
        Err(format!(
            "unknown skill '{name}'. Available: {}",
            if names.is_empty() {
                "(none)".to_string()
            } else {
                names.join(", ")
            }
        ))
    }
}
