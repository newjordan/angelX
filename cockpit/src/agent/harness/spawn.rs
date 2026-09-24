//! Spawn Formations — the agent summons N copies of itself (or fleet peers)
//! mid-turn, in a *formation*, each seat dressed with a callable *persona* and
//! a *tool grant*. Nothing is pre-loaded: persona bodies live on disk and are
//! read at spawn time (the skills model — catalog visible, cost on demand),
//! and each grant builds a purpose-scoped registry so a six-seat panel never
//! pays six MCP/LSP inits.
//!
//! This is the missing middle between `delegate` (one specialist, one git
//! worktree, serial) and the swarm driver (a fixed MoA pipeline the *user*
//! selects): `spawn` lets the agent itself decide, mid-turn, to become many.
//!
//! Formations:
//!   solo   — one seat; a cheap sidebar consultation.
//!   panel  — N seats in parallel; every answer returned, labeled by persona.
//!   moa    — N parallel drafts folded by one synthesis call (answer + seats).
//!   quorum — first K successful seats to land win; stragglers are cut.
//!
//! Seats run detached with results over a channel (the bounded-collection
//! shape the swarm's fan-out uses), so one hung endpoint can't pin the
//! formation past its deadline; a formation-wide cancel flag stops cut seats
//! at their next hop boundary. Recursion is depth-gated: under Treebeard,
//! seats may re-`spawn` while `subcall_depth < max_depth`; the default lane
//! still forbids seat-level spawn (`max_depth=1`).

use super::independence::{self, ChildFootprint};
use super::*;

const SPAWN_MAX_DEFAULT: usize = 8;
const SPAWN_INFLIGHT_FLOOR: usize = 64;
static SPAWN_SEATS_INFLIGHT: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct SpawnSeatPermit;

impl Drop for SpawnSeatPermit {
    fn drop(&mut self) {
        SPAWN_SEATS_INFLIGHT.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Detached threads cannot be force-killed safely. Bound their total process
/// footprint instead: a blocked provider may retain its seat, but repeated
/// timed-out formations cannot create an unbounded number of blocked threads.
pub(crate) fn reserve_spawn_seats(n: usize, limit: usize) -> Result<Vec<SpawnSeatPermit>, String> {
    let limit = limit.max(1);
    let mut current = SPAWN_SEATS_INFLIGHT.load(Ordering::Acquire);
    loop {
        let Some(next) = current.checked_add(n) else {
            return Err("spawn seat counter overflow".to_string());
        };
        if next > limit {
            return Err(format!(
                "spawn capacity exhausted: {current} seat(s) are still running, {n} requested, process limit {limit}"
            ));
        }
        match SPAWN_SEATS_INFLIGHT.compare_exchange_weak(
            current,
            next,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return Ok((0..n).map(|_| SpawnSeatPermit).collect()),
            Err(observed) => current = observed,
        }
    }
}

#[cfg(test)]
pub(crate) fn spawn_seats_inflight() -> usize {
    SPAWN_SEATS_INFLIGHT.load(Ordering::Acquire)
}

/// Fold MoA drafts without letting aggregation extend the formation deadline.
/// The worker owns a seat permit until the provider call *actually* exits. If
/// cancellation is only cooperative and the club blocks, the caller still
/// returns on time and admission control prevents unbounded replacement
/// synthesis threads.
#[cfg(test)]
pub(crate) fn bounded_moa_synthesis(
    club: Arc<dyn Club>,
    messages: Vec<ChatMsg>,
    budget: Duration,
    inflight_limit: usize,
) -> Option<String> {
    bounded_moa_synthesis_with_deadline(club, messages, Some(budget), inflight_limit)
}

fn bounded_moa_synthesis_with_deadline(
    club: Arc<dyn Club>,
    messages: Vec<ChatMsg>,
    budget: Option<Duration>,
    inflight_limit: usize,
) -> Option<String> {
    if budget.is_some_and(|duration| duration.is_zero()) {
        return None;
    }
    let deadline = budget.and_then(|duration| Instant::now().checked_add(duration));
    let permit = loop {
        match reserve_spawn_seats(1, inflight_limit) {
            Ok(mut permits) => break permits.pop().expect("one permit requested"),
            Err(_) => {
                if let Some(deadline) = deadline {
                    let remaining = deadline.checked_duration_since(Instant::now())?;
                    if remaining.is_zero() {
                        return None;
                    }
                    std::thread::sleep(remaining.min(Duration::from_millis(5)));
                } else {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("spawn-moa-synthesis".into())
        .spawn(move || {
            let _permit = permit;
            let result = club.chat_streaming(&messages, &[], &worker_cancel, &mut |_| {});
            let _ = tx.send(result);
        })
        .is_ok();
    if !spawned {
        return None;
    }
    if let Some(deadline) = deadline {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            cancel.store(true, Ordering::Release);
            return None;
        };
        return match rx.recv_timeout(remaining) {
            Ok(Ok(crate::agent::club::ClubReply::Text(answer))) if !answer.trim().is_empty() => {
                Some(answer)
            }
            Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => None,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                cancel.store(true, Ordering::Release);
                None
            }
        };
    };
    match rx.recv() {
        Ok(Ok(crate::agent::club::ClubReply::Text(answer))) if !answer.trim().is_empty() => {
            Some(answer)
        }
        Ok(_) | Err(_) => None,
    }
}

/// Personas share the skill frontmatter/body grammar, but deliberately have a
/// separate filename namespace: `<name>/PERSONA.md` is canonical, with legacy
/// `<name>/SKILL.md` and flat `<name>.md` accepted for existing user bundles.
/// The shipped set is embedded so installing or relocating the binary cannot
/// silently empty the advertised catalog. Explicit bundled and user directories
/// overlay it case-insensitively; the user layer wins last.
pub(crate) fn load_personas() -> Vec<Skill> {
    let embedded = [
        (
            "architect",
            include_str!("../../../personas/architect/PERSONA.md"),
        ),
        (
            "code-help",
            include_str!("../../../personas/code-help/PERSONA.md"),
        ),
        (
            "librarian",
            include_str!("../../../personas/librarian/PERSONA.md"),
        ),
        (
            "reviewer",
            include_str!("../../../personas/reviewer/PERSONA.md"),
        ),
        (
            "security",
            include_str!("../../../personas/security/PERSONA.md"),
        ),
        (
            "skeptic",
            include_str!("../../../personas/skeptic/PERSONA.md"),
        ),
        (
            "speed-freak",
            include_str!("../../../personas/speed-freak/PERSONA.md"),
        ),
        (
            "treebeard",
            include_str!("../../../personas/treebeard/PERSONA.md"),
        ),
    ]
    .into_iter()
    .map(|(fallback, text)| parse_skill(text, fallback))
    .collect::<Vec<_>>();
    let bundled = std::env::var_os("ANGEL_BUNDLED_PERSONAS_DIR")
        .map(PathBuf::from)
        .map(|dir| load_personas_from(&dir))
        .unwrap_or_default();
    let user = std::env::var_os("ANGEL_PERSONAS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".angelX/personas")
        });
    merge_personas(merge_personas(embedded, bundled), load_personas_from(&user))
}

fn merge_personas(base: Vec<Skill>, overlay: Vec<Skill>) -> Vec<Skill> {
    let mut by_name = std::collections::BTreeMap::new();
    for persona in base.into_iter().chain(overlay) {
        by_name.insert(persona.name.to_ascii_lowercase(), persona);
    }
    by_name.into_values().collect()
}

/// Load only persona-shaped bundles. Generic skill discovery must not learn to
/// ingest `PERSONA.md`, because that would leak persona bodies into skill
/// catalogs and automatic skill routing.
pub(crate) fn load_personas_from(dir: &Path) -> Vec<Skill> {
    let mut entries = match confined_read_dir(dir, Path::new("")) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    let mut personas = Vec::new();
    for entry in entries.into_iter().take(MAX_SKILLS_PER_DIR) {
        let entry_path = PathBuf::from(&entry.name);
        let (relative, fallback) = if entry.is_dir {
            let canonical = entry_path.join("PERSONA.md");
            let legacy = entry_path.join("SKILL.md");
            // Presence selects the canonical file even if its eventual confined
            // open fails. Never fall through from a malformed/unsafe canonical
            // bundle to a different legacy body in the same directory.
            let has_canonical = confined_read_dir(dir, &entry_path)
                .ok()
                .is_some_and(|children| {
                    children
                        .iter()
                        .any(|child| child.name.as_os_str() == std::ffi::OsStr::new("PERSONA.md"))
                });
            let relative = if has_canonical { canonical } else { legacy };
            (relative, entry.name.to_string_lossy().to_string())
        } else if entry_path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            let fallback = entry_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("")
                .to_string();
            if fallback.eq_ignore_ascii_case("readme") {
                continue;
            }
            (entry_path, fallback)
        } else {
            continue;
        };
        let Ok(Some(bytes)) = confined_read_limited(dir, &relative, MAX_SKILL_FILE_BYTES) else {
            continue;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let persona = parse_skill(&text, &fallback);
        if safe_skill_name(&persona.name) {
            personas.push(persona);
        }
    }
    personas.sort_by(|left, right| {
        left.name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
    });
    personas
}

pub(crate) fn plain_persona_alias(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "none" | "plain" | "default"
    )
}

pub(crate) fn requested_personas(value: Option<&Value>) -> Result<Vec<String>, String> {
    let normalize = |raw: &str| {
        let trimmed = raw.trim();
        (!trimmed.is_empty() && !plain_persona_alias(trimmed)).then(|| trimmed.to_string())
    };
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(raw)) => Ok(normalize(raw).into_iter().collect()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value.as_str().ok_or_else(|| {
                    "persona list entries must be strings; use an exact installed name or omit `persona` for a plain seat".to_string()
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|values| values.into_iter().filter_map(normalize).collect()),
        Some(_) => Err(
            "persona must be a string or string array; use an exact installed name or omit `persona` for a plain seat"
                .to_string(),
        ),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Formation {
    Solo,
    Panel,
    Moa,
    Quorum,
}

impl Formation {
    fn parse(raw: Option<&str>, n: usize) -> Result<Self, String> {
        match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            None | Some("") => Ok(if n > 1 { Self::Panel } else { Self::Solo }),
            Some("solo") => Ok(Self::Solo),
            Some("panel") => Ok(Self::Panel),
            Some("moa") | Some("waves") => Ok(Self::Moa),
            Some("quorum") => Ok(Self::Quorum),
            Some(other) => Err(format!(
                "unknown formation '{other}'; expected solo, panel, moa, or quorum"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Solo => "solo",
            Self::Panel => "panel",
            Self::Moa => "moa",
            Self::Quorum => "quorum",
        }
    }
}

/// Shared with the agent-graph runtime — graph node seats use the same scoped
/// tool grants as spawn seats.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Grant {
    None,
    ReadOnly,
    Code,
    Research,
}

impl Grant {
    const READ_WORKSPACE: u8 = 1 << 0;
    const WRITE_WORKSPACE: u8 = 1 << 1;
    const LOCAL_EXEC: u8 = 1 << 2;
    const NETWORK_RESEARCH: u8 = 1 << 3;

    pub(crate) fn parse(raw: Option<&str>) -> Result<Self, String> {
        match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            None | Some("") | Some("read_only") | Some("readonly") => Ok(Self::ReadOnly),
            Some("none") => Ok(Self::None),
            Some("code") | Some("write") => Ok(Self::Code),
            Some("research") | Some("web") => Ok(Self::Research),
            Some(other) => Err(format!(
                "unknown tools grant '{other}'; expected none, read_only, code, or research"
            )),
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ReadOnly => "read_only",
            Self::Code => "code",
            Self::Research => "research",
        }
    }

    /// Capability-set semantics for nested delegation. Grants are deliberately
    /// not ordered: `read_only` includes local shell inspection while
    /// `research` includes network access, so neither is a subset of the other.
    fn capabilities(self) -> u8 {
        match self {
            Self::None => 0,
            Self::ReadOnly => Self::READ_WORKSPACE | Self::LOCAL_EXEC,
            Self::Code => Self::READ_WORKSPACE | Self::WRITE_WORKSPACE | Self::LOCAL_EXEC,
            Self::Research => Self::READ_WORKSPACE | Self::NETWORK_RESEARCH,
        }
    }

    fn is_subset_of(self, ceiling: Self) -> bool {
        self.capabilities() & !ceiling.capabilities() == 0
    }

    /// One scoped registry per *formation* (tools are `Send + Sync`; seats
    /// dispatch through a shared `Arc`). Deliberately lean — no MCP, no LSP —
    /// so a wide panel starts in microseconds. Nested `spawn` is optional
    /// under Treebeard when depth still allows another nesting level.
    pub(crate) fn registry(
        self,
        workspace: &Path,
        nested_spawn: Option<&SpawnTool>,
        cargo: &PinnedCargo,
    ) -> ToolRegistry {
        let mut r = ToolRegistry::new();
        // Keep registry-level policy/evidence rooted to the same workspace as
        // the scoped tools. Without this, grant registries inherited the
        // process cwd, so finalization fingerprints and Atlas state could
        // inspect a different repository from the seat's actual tools.
        r.set_workspace(workspace.to_path_buf());
        match self {
            Self::None => {}
            Self::ReadOnly => {
                r.register(Box::new(
                    ShellTool::in_dir(workspace.to_path_buf())
                        .with_mutation_targets(Arc::clone(&r.mutation_targets)),
                ));
                register_read_tools(&mut r, workspace);
            }
            Self::Code => {
                r.register(Box::new(
                    ShellTool::in_dir(workspace.to_path_buf())
                        .with_mutation_targets(Arc::clone(&r.mutation_targets)),
                ));
                r.register(Box::new(
                    CargoTool::in_dir_with_cargo(workspace.to_path_buf(), cargo.clone())
                        .with_mutation_targets(Arc::clone(&r.mutation_targets)),
                ));
                register_file_tools(&mut r, workspace.to_path_buf());
            }
            Self::Research => {
                register_read_tools(&mut r, workspace);
                maybe_register_web_search(&mut r);
                maybe_register_web_fetch(&mut r);
                maybe_register_science(&mut r);
                maybe_register_repos(&mut r);
            }
        }
        if let Some(parent) = nested_spawn {
            // Child formation: same workspace/roster, bounded to this seat's
            // exact capability set. The root tool is unrestricted; only this
            // derived nested handle carries a ceiling.
            r.register(Box::new(SpawnTool {
                workspace: parent.workspace.clone(),
                self_club: parent.self_club.clone(),
                clubs: parent.clubs.clone(),
                personas: parent.personas.clone(),
                grant_ceiling: Some(self),
                cargo: parent.cargo.clone(),
            }));
        }
        r
    }
}

/// The read-only slice of the file toolset (no write/patch/repair).
fn register_read_tools(r: &mut ToolRegistry, root: &Path) {
    let root = root.to_path_buf();
    r.register(Box::new(ReadFileTool { root: root.clone() }));
    r.register(Box::new(GrepTool { root: root.clone() }));
    r.register(Box::new(FindFilesTool { root: root.clone() }));
    r.register(Box::new(OutlineTool { root: root.clone() }));
    r.register(Box::new(ListDirTool { root }));
}

struct SeatResult {
    seat: usize,
    persona: String,
    club: String,
    elapsed_ms: u128,
    answer: Result<String, String>,
}

/// The `spawn` tool: formation fan-out over self-copies or fleet peers.
pub(crate) struct SpawnTool {
    workspace: PathBuf,
    /// The caller's own club as of registry build — what `club:"self"` means.
    /// `None` (or a stale in-hand after a Tab) degrades to `auto`.
    self_club: Option<Arc<dyn Club>>,
    clubs: HashMap<String, Arc<dyn Club>>,
    personas: Vec<Skill>,
    /// `None` only for the root interactive tool. Nested handles carry the
    /// parent seat's effective grant and can request only a capability subset.
    grant_ceiling: Option<Grant>,
    cargo: PinnedCargo,
}

impl SpawnTool {
    #[cfg(test)]
    pub(crate) fn new(
        workspace: PathBuf,
        self_club: Option<Arc<dyn Club>>,
        roster: Vec<Arc<dyn Club>>,
    ) -> Self {
        let cargo = PinnedCargo::capture(&workspace);
        Self::new_with_cargo(workspace, self_club, roster, cargo)
    }

    pub(crate) fn new_with_cargo(
        workspace: PathBuf,
        self_club: Option<Arc<dyn Club>>,
        roster: Vec<Arc<dyn Club>>,
        cargo: PinnedCargo,
    ) -> Self {
        let clubs = roster
            .into_iter()
            .map(|c| (c.label().to_lowercase(), c))
            .collect();
        Self {
            workspace,
            self_club,
            clubs,
            personas: load_personas(),
            grant_ceiling: None,
            cargo,
        }
    }

    fn enforce_grant_ceiling(&self, requested: Grant) -> Result<(), String> {
        let Some(ceiling) = self.grant_ceiling else {
            return Ok(());
        };
        if requested.is_subset_of(ceiling) {
            return Ok(());
        }
        Err(format!(
            "nested spawn tools={} exceeds parent grant ceiling tools={} (delegation capabilities may stay the same or narrow, never escalate)",
            requested.label(),
            ceiling.label()
        ))
    }

    fn default_grant(&self) -> Grant {
        if self
            .grant_ceiling
            .is_none_or(|ceiling| Grant::ReadOnly.is_subset_of(ceiling))
        {
            Grant::ReadOnly
        } else {
            Grant::None
        }
    }

    #[cfg(test)]
    pub(crate) fn nested_for_test(&self, parent_grant: Grant) -> Self {
        Self {
            workspace: self.workspace.clone(),
            self_club: self.self_club.clone(),
            clubs: self.clubs.clone(),
            personas: self.personas.clone(),
            grant_ceiling: Some(parent_grant),
            cargo: self.cargo.clone(),
        }
    }

    /// Resolve the club for each seat. `self`/`auto` = the caller's own club
    /// replicated (self-same panel — the quality floor is the driver itself,
    /// never a random fleet box); `smart`/`sota` = the ONE designated
    /// escalation seat (`ANGEL_SOTA_SMART_CLUB`, default `luna`); `fleet`
    /// explicitly spreads seats round-robin across reachable roster clubs (the
    /// old `auto` — it silently seated cheap test-fleet models into helper
    /// panels and wrecked output quality, so it is now opt-in by name); a
    /// label picks one club explicitly.
    fn seat_clubs(&self, spec: &str, n: usize) -> Result<Vec<Arc<dyn Club>>, String> {
        let mut spec = spec.trim().to_lowercase();
        if spec == "smart" || spec == "sota" {
            // The designated escalation seat (Sol@max default, Kimi-K3@high
            // alternative; TUI operators get the popup choice). Availability
            // fallback: if the primary is not seated here, try the alternative.
            // Pin the effort on an immutable per-seat view. The underlying
            // route can still serve ordinary turns and graph nodes concurrently
            // without a last-writer-wins reasoning setting.
            let seat = crate::agent::club::smart_seat();
            let seat = if self.clubs.contains_key(&seat.club.to_lowercase()) {
                seat
            } else {
                crate::agent::club::smart_seat_alt()
            };
            spec = seat.club.to_lowercase();
            if let Some(club) = self.clubs.get(&spec) {
                let (club, _) =
                    crate::agent::club::scoped_reasoning_effort(Arc::clone(club), &seat.effort);
                return Ok(vec![club; n]);
            }
        }
        if (spec == "self" || spec == "auto" || spec.is_empty())
            && let Some(c) = &self.self_club
        {
            return Ok(vec![Arc::clone(c); n]);
        }
        // No pinned self — fall through to the fleet spread rather than failing.
        if spec == "fleet" || spec == "auto" || spec == "self" || spec.is_empty() {
            let live: Vec<&Arc<dyn Club>> =
                self.clubs.values().filter(|c| c.is_available()).collect();
            if live.is_empty() {
                return Err("no fleet club is currently reachable".to_string());
            }
            return Ok((0..n).map(|i| Arc::clone(live[i % live.len()])).collect());
        }
        match self.clubs.get(&spec) {
            Some(c)
                if crate::agent::tools::consult::is_optional_local_label(c.label())
                    && !c.is_available() =>
            {
                let live: Vec<&Arc<dyn Club>> = self
                    .clubs
                    .values()
                    .filter(|peer| {
                        crate::agent::tools::consult::is_optional_local_label(peer.label())
                            && peer.is_available()
                    })
                    .collect();
                if !live.is_empty() {
                    return Ok((0..n).map(|i| Arc::clone(live[i % live.len()])).collect());
                }
                if let Some(self_c) = &self.self_club {
                    return Ok(vec![Arc::clone(self_c); n]);
                }
                Err(format!(
                    "club '{spec}' is not reachable and no live local/self seat is available"
                ))
            }
            Some(c) => Ok(vec![Arc::clone(c); n]),
            None if crate::agent::club::is_sota_label(&spec)
                && (crate::agent::tools::solo::solo_mode_active()
                    || !crate::agent::harness::env_flag("ANGEL_ALLOW_SOTA_DELEGATE", true)) =>
            {
                if crate::agent::tools::solo::solo_mode_active() {
                    Err(format!(
                        "solo mode: refuse spawn to paid SOTA seat '{spec}'. Do the work yourself. \
                         Available now: self, auto, {}",
                        self.sorted_club_names().join(", ")
                    ))
                } else {
                    Err(format!(
                        "club '{spec}' is a paid SOTA seat withheld from spawn because \
                         ANGEL_ALLOW_SOTA_DELEGATE=0 (local-only fleet opt-out). Unset that pin or set \
                         ANGEL_ALLOW_SOTA_DELEGATE=1. Available now: self, auto, {}",
                        self.sorted_club_names().join(", ")
                    ))
                }
            }
            None => Err(format!(
                "unknown club '{spec}'; have: self, auto, {}",
                self.sorted_club_names().join(", ")
            )),
        }
    }

    fn sorted_club_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.clubs.keys().cloned().collect();
        names.sort();
        names
    }

    /// Persona for seat `i`: an explicit list cycles across seats; a single
    /// name dresses every seat; none = a plain numbered seat. Bodies are read
    /// from the catalog loaded at registry build — never the caller's context.
    fn seat_persona(&self, personas: &[String], i: usize) -> Result<(String, String), String> {
        if personas.is_empty() {
            return Ok((format!("seat-{}", i + 1), String::new()));
        }
        let want = &personas[i % personas.len()];
        self.personas
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(want))
            .map(|p| (p.name.clone(), p.body.clone()))
            .ok_or_else(|| {
                let names: Vec<&str> = self.personas.iter().map(|p| p.name.as_str()).collect();
                format!(
                    "unknown persona '{want}'. Available: {}. Use an exact installed name, or omit `persona` for a plain seat",
                    if names.is_empty() {
                        "(none installed)".to_string()
                    } else {
                        names.join(", ")
                    }
                )
            })
    }

    #[allow(clippy::too_many_arguments)]
    fn run_formation(
        &self,
        formation: Formation,
        tasks: &[String],
        personas: &[String],
        grant: Grant,
        club_spec: &str,
        timeout: Duration,
        quorum_k: usize,
    ) -> Result<String, String> {
        // Direct test/tool invocations get a root allowance here; ordinary
        // turn dispatch already installed one, and nested seats inherit it.
        let _budget_scope = DescendantBudgetScope::enter_root();
        let descendant_budget = current_descendant_budget()?;
        let started = Instant::now();
        // Approval posture controls authority, never behavioral runaway.
        // A blocking provider must not turn a bounded formation into an
        // unbounded parent wait merely because YOLO is enabled.
        let deadline = (!timeout.is_zero()).then_some(timeout);
        let n = tasks.len();
        let clubs = self.seat_clubs(club_spec, n)?;
        // Resolve every persona before starting any thread. Previously an
        // invalid later persona returned early after earlier seats had already
        // detached, without ever setting their cancellation flag.
        let seat_personas = (0..n)
            .map(|i| self.seat_persona(personas, i))
            .collect::<Result<Vec<_>, _>>()?;
        // A formation is one atomic logical wave. MoA's aggregator is itself
        // a model descendant, so reserve it explicitly with the drafts even
        // if a later timeout/cancellation prevents that admitted call.
        let descendant_calls = n
            .checked_add(usize::from(formation == Formation::Moa))
            .ok_or("descendant call count overflow")?;
        let budget_receipt = reserve_descendant_calls(
            &descendant_budget,
            descendant_calls,
            &format!("spawn {}", formation.label()),
        )?;
        let configured_max = env_usize("ANGEL_SPAWN_MAX", SPAWN_MAX_DEFAULT).max(1);
        let inflight_limit = env_usize(
            "ANGEL_SPAWN_INFLIGHT_MAX",
            SPAWN_INFLIGHT_FLOOR.max(configured_max),
        )
        .max(configured_max);
        let permits = reserve_spawn_seats(n, inflight_limit)
            .map_err(|error| format!("{error}; {}", budget_receipt.fields()))?;
        // Child seats run at depth+1. Allow them to re-spawn only when that
        // level is still below the lane max (Treebeard default 2).
        let nested_spawn = (subcall_depth() + 1 < treebeard_max_depth()).then_some(self);
        // One shared registry per formation; seats dispatch concurrently.
        let registry = Arc::new(grant.registry(&self.workspace, nested_spawn, &self.cargo));
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel::<SeatResult>();

        let seat_labels = seat_personas
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for (i, ((persona_name, persona_body), permit)) in
            seat_personas.into_iter().zip(permits).enumerate()
        {
            let system = seat_system(&persona_name, &persona_body, formation, n, grant);
            let task = tasks[i].clone();
            let club = Arc::clone(&clubs[i]);
            let club_label = club.label().to_string();
            let registry = Arc::clone(&registry);
            let cancel = Arc::clone(&cancel);
            let seat_tx = tx.clone();
            let failed_tx = tx.clone();
            let failed_persona = persona_name.clone();
            let thread_budget = descendant_budget.clone();
            if let Err(err) = std::thread::Builder::new()
                .name(format!("spawn-seat-{}", i + 1))
                .spawn(move || {
                    let _permit = permit;
                    let _budget = DescendantBudgetScope::inherit(thread_budget);
                    let _depth = SubcallDepthGuard::enter();
                    let started = Instant::now();
                    let mut history = vec![
                        ChatMsg::system(system.as_str()),
                        ChatMsg::user(task.as_str()),
                    ];
                    // Seat traces have no UI consumer yet (same as delegate).
                    let (evt_tx, _) = mpsc::channel::<TurnEvent>();
                    let answer = run_turn(
                        &*club,
                        &registry,
                        &mut history,
                        &cancel,
                        Some(default_max_hops()),
                        &evt_tx,
                    );
                    let _ = seat_tx.send(SeatResult {
                        seat: i,
                        persona: persona_name,
                        club: club.label().to_string(),
                        elapsed_ms: started.elapsed().as_millis(),
                        answer,
                    });
                })
            {
                // `spawn` drops the closure (and its permit) on failure. Land a
                // synthetic seat result immediately instead of waiting for the
                // full formation deadline for a thread that never existed.
                let _ = failed_tx.send(SeatResult {
                    seat: i,
                    persona: failed_persona,
                    club: club_label,
                    elapsed_ms: 0,
                    answer: Err(format!("seat thread failed to spawn: {err}")),
                });
            }
        }
        drop(tx);
        crate::ui::viz::agentviz::stage(format!("spawn {}", formation.label()), seat_labels);

        // Collect until: all seats, a successful quorum, a quorum that can no
        // longer be reached, or the deadline. Failed seats still land, but do
        // not satisfy quorum and therefore cannot cut slower healthy seats.
        // Cut seats get the formation cancel flag flipped and wind down at
        // their next hop boundary in the background.
        let mut results: Vec<Option<SeatResult>> = (0..n).map(|_| None).collect();
        let mut landed = 0usize;
        let mut successful = 0usize;
        while landed < n {
            if formation == Formation::Quorum
                && (successful >= quorum_k || successful + (n - landed) < quorum_k)
            {
                break;
            }
            let received = if let Some(limit) = deadline {
                let Some(remaining) = limit.checked_sub(started.elapsed()) else {
                    break;
                };
                match rx.recv_timeout(remaining.min(Duration::from_millis(200))) {
                    Ok(result) => Ok(result),
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => Err(()),
                }
            } else {
                rx.recv().map_err(|_| ())
            };
            match received {
                Ok(r) => {
                    let idx = r.seat;
                    if results[idx].is_none() {
                        landed += 1;
                        successful += usize::from(r.answer.is_ok());
                    }
                    results[idx] = Some(r);
                }
                Err(()) => break,
            }
        }
        cancel.store(true, Ordering::Relaxed);

        let seats: Vec<SeatResult> = results.into_iter().flatten().collect();
        if formation == Formation::Quorum && successful < quorum_k {
            let failures = seats
                .iter()
                .filter_map(|seat| {
                    seat.answer
                        .as_ref()
                        .err()
                        .map(|error| format!("{}: {error}", seat.persona))
                })
                .collect::<Vec<_>>();
            let detail = if failures.is_empty() {
                String::new()
            } else {
                format!(": {}", failures.join("; "))
            };
            return Err(format!(
                "formation quorum not reached ({successful}/{quorum_k} successful; {landed}/{n} seats landed){detail}; {}",
                descendant_budget_status(&descendant_budget)?.status_fields()
            ));
        }
        if seats.iter().all(|s| s.answer.is_err()) {
            let errs: Vec<String> = seats
                .iter()
                .map(|s| {
                    format!(
                        "{}: {}",
                        s.persona,
                        s.answer.as_ref().err().cloned().unwrap_or_default()
                    )
                })
                .collect();
            return Err(format!(
                "formation returned no answers ({landed}/{n} seats landed): {}; {}",
                errs.join("; "),
                descendant_budget_status(&descendant_budget)?.status_fields()
            ));
        }
        let elapsed = started.elapsed();
        let synthesis_budget = deadline.map(|limit| limit.saturating_sub(elapsed));
        self.digest(
            formation,
            tasks,
            grant,
            n,
            landed,
            elapsed,
            synthesis_budget,
            inflight_limit,
            seats,
            &descendant_budget,
            budget_receipt.requested,
        )
    }

    #[cfg(test)]
    pub(crate) fn run_moa_for_test(
        &self,
        tasks: &[String],
        timeout: Duration,
    ) -> Result<String, String> {
        self.run_formation(Formation::Moa, tasks, &[], Grant::None, "self", timeout, 1)
    }

    #[allow(clippy::too_many_arguments)]
    fn digest(
        &self,
        formation: Formation,
        tasks: &[String],
        grant: Grant,
        n: usize,
        landed: usize,
        elapsed: Duration,
        synthesis_budget: Option<Duration>,
        inflight_limit: usize,
        seats: Vec<SeatResult>,
        descendant_budget: &DescendantBudgetHandle,
        admitted_descendant_calls: usize,
    ) -> Result<String, String> {
        let mut budget_status = descendant_budget_status(descendant_budget)?;
        budget_status.requested = admitted_descendant_calls;
        let header = format!(
            "[spawn formation={} seats={landed}/{n} grant={} elapsed={}s {}]",
            formation.label(),
            grant.label(),
            elapsed.as_secs(),
            budget_status.fields(),
        );
        let seat_block = |s: &SeatResult, cap: usize| -> String {
            let body = match &s.answer {
                Ok(a) => cap_chars(a.trim(), cap),
                Err(e) => format!("(seat failed: {e})"),
            };
            format!(
                "### {} · {} · {:.1}s\n{}",
                s.persona,
                s.club,
                s.elapsed_ms as f64 / 1000.0,
                body
            )
        };
        match formation {
            Formation::Solo | Formation::Panel | Formation::Quorum => {
                let cap = 3_500;
                let blocks: Vec<String> = seats.iter().map(|s| seat_block(s, cap)).collect();
                let body = blocks.join("\n\n");
                Ok(offload_spawn_root(&header, &body, formation.label()))
            }
            Formation::Moa => {
                // Fold the drafts with one synthesis call on the first seat's
                // club (the formation is homogeneous unless auto-routed; any
                // member is a fair synthesizer).
                let drafts: Vec<&SeatResult> = seats.iter().filter(|s| s.answer.is_ok()).collect();
                let synth_club = self
                    .seat_clubs("self", 1)
                    .ok()
                    .and_then(|v| v.into_iter().next())
                    .or_else(|| {
                        drafts
                            .first()
                            .and_then(|s| self.clubs.get(&s.club.to_lowercase()).cloned())
                    });
                let task = tasks.first().cloned().unwrap_or_default();
                let joined: String = drafts
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        format!(
                            "--- draft {} ({}) ---\n{}\n",
                            i + 1,
                            s.persona,
                            cap_chars(s.answer.as_ref().unwrap().trim(), 6_000)
                        )
                    })
                    .collect();
                let folded = synth_club.and_then(|club| {
                    let msgs = vec![
                        ChatMsg::system(
                            "You are the aggregator of a spawn formation. Fold the drafts \
                             into one best answer: keep every well-supported point, drop \
                             contradictions and filler, resolve disagreements explicitly. \
                             Output only the final answer.",
                        ),
                        ChatMsg::user(format!("Task:\n{task}\n\nDrafts:\n{joined}")),
                    ];
                    bounded_moa_synthesis_with_deadline(
                        club,
                        msgs,
                        synthesis_budget,
                        inflight_limit,
                    )
                });
                match folded {
                    Some(answer) => {
                        let stats: Vec<String> = seats
                            .iter()
                            .map(|s| {
                                format!(
                                    "{} {:.1}s{}",
                                    s.persona,
                                    s.elapsed_ms as f64 / 1000.0,
                                    if s.answer.is_err() { " ✗" } else { "" }
                                )
                            })
                            .collect();
                        let strat = format!("{header}\n{}", stats.join(" · "));
                        Ok(offload_spawn_root(&strat, answer.trim(), "moa"))
                    }
                    // Synthesis failed — degrade to a panel digest rather than
                    // discarding the drafts the fleet already paid for.
                    None => {
                        let blocks: Vec<String> =
                            seats.iter().map(|s| seat_block(s, 3_500)).collect();
                        let body = blocks.join("\n\n");
                        let strat = format!("{header}\n(synthesis unavailable — raw drafts)");
                        Ok(offload_spawn_root(&strat, &body, "moa-drafts"))
                    }
                }
            }
        }
    }
}

/// Keep the formation strategy line(s) root-visible; offload bulk seat digests
/// under a session handle when they clear the subcall floor.
fn offload_spawn_root(strategy: &str, bulk: &str, identity_tag: &str) -> String {
    let min = subcall_offload_min_bytes();
    if let Some(receipt) = maybe_offload_root_body(
        bulk,
        HandleKind::Subcall,
        "spawn",
        &format!("spawn|{identity_tag}"),
        min,
    ) {
        format!("{strategy}\n{receipt}")
    } else if bulk.is_empty() {
        strategy.to_string()
    } else {
        format!("{strategy}\n\n{bulk}")
    }
}

fn seat_system(
    persona_name: &str,
    persona_body: &str,
    formation: Formation,
    n: usize,
    grant: Grant,
) -> String {
    let mut s = format!(
        "You are one seat of an Angel spawn formation ({} of {n} independent agents \
         working the same task in parallel — formation: {}). This is a bounded \
         consultation, not an implementation turn. Work alone; do not \
         reference other seats. Return your best complete result as plain text.",
        persona_name,
        formation.label()
    );
    if !persona_body.trim().is_empty() {
        s.push_str("\n\nYour persona — inhabit it fully:\n");
        s.push_str(persona_body.trim());
    }
    if grant == Grant::None {
        s.push_str(
            "\n\nYou have no tools this run: answer from reasoning alone. \
             Tool use and workspace mutation are intentionally out of scope \
             for this consult; return the completed answer directly.",
        );
    } else {
        s.push_str("\n\n");
        s.push_str(super::TOOL_BATCHING_HINT);
    }
    s
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/spawn__prompt_tests.rs"]
mod prompt_tests;

fn cap_chars(s: &str, cap: usize) -> String {
    if s.chars().count() <= cap {
        return s.to_string();
    }
    let head: String = s.chars().take(cap).collect();
    format!("{head}\n…[seat answer truncated]")
}

impl Tool for SpawnTool {
    fn workspace_write_scope_is_opaque(&self, args: &Value) -> bool {
        let grant = match args.get("tools").and_then(Value::as_str) {
            Some(raw) => match Grant::parse(Some(raw)) {
                Ok(grant) => grant,
                Err(_) => return false,
            },
            None => self.default_grant(),
        };
        grant == Grant::Code && self.enforce_grant_ceiling(grant).is_ok()
    }

    fn name(&self) -> &str {
        "spawn"
    }

    fn def(&self) -> ToolDef {
        let personas: Vec<&str> = self.personas.iter().map(|p| p.name.as_str()).collect();
        let persona_string_options = personas.clone();
        let persona_array_options = personas.clone();
        let mut clubs: Vec<&str> = self.clubs.keys().map(String::as_str).collect();
        clubs.sort();
        let grant_options = [Grant::None, Grant::ReadOnly, Grant::Code, Grant::Research]
            .into_iter()
            .filter(|requested| {
                self.grant_ceiling
                    .is_none_or(|ceiling| requested.is_subset_of(ceiling))
            })
            .map(Grant::label)
            .collect::<Vec<_>>();
        // Keep the historical read-only default when it fits. Research and
        // none ceilings default to no tools because read-only local shell is
        // not a subset of either capability set.
        let default_grant = self.default_grant().label();
        let ceiling = self
            .grant_ceiling
            .map(|grant| {
                format!(
                    " This nested handle is capped at the parent tools={} capability set; child grants may only stay equal or narrow.",
                    grant.label()
                )
            })
            .unwrap_or_default();
        ToolDef {
            name: "spawn".to_string(),
            description: format!(
                "Spawn a formation of parallel sub-agents mid-turn: copies of yourself \
                 (club=self or auto — the default and the quality floor), the designated smart \
                 escalation seat (club=smart), or an explicit fleet spread (club=fleet, opt-in). \
                 Formations: solo (one seat), \
                 panel (all answers back, labeled), moa (drafts folded into one answer), quorum (first K win). \
                 Each seat can wear one exact installed persona \
                 and gets a tool grant. Installed personas: {}. Clubs: self, auto, {}. \
                 For direct single-call model questions use `consult_model` or `code_review`; \
                 for git-isolated implementation use `delegate`.{}",
                if personas.is_empty() {
                    "(none installed)".to_string()
                } else {
                    personas.join(", ")
                },
                clubs.join(", "),
                ceiling,
            ),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "task": { "type": "string", "description": "the task every seat works (or use tasks[])" },
                    "tasks": { "type": "array", "items": { "type": "string" },
                               "description": "one task per seat (sets n)" },
                    "formation": { "type": "string", "enum": ["solo", "panel", "moa", "quorum"],
                                   "description": "default: panel when n>1, else solo" },
                    "n": { "type": "integer", "description": "seat count (default 3, capped by ANGEL_SPAWN_MAX)" },
                    "persona": {
                        "oneOf": [
                            { "type": "string", "enum": persona_string_options },
                            { "type": "array", "items": { "type": "string", "enum": persona_array_options } }
                        ],
                        "description": format!("optional exact persona name or list cycling across seats; installed names: {}; omit this field for a plain seat", personas.join(", "))
                    },
                    "tools": { "type": "string", "enum": grant_options,
                               "default": default_grant,
                               "description": "seat tool grant; code requires n=1 (use delegate for parallel writes)" },
                    "club": { "type": "string", "description": "self/auto = your own model replicated (default, self-same panel) | smart = the designated escalation seat (ANGEL_SOTA_SMART_CLUB, default luna) | fleet = spread across reachable fleet clubs (opt-in) | an explicit club label" },
                    "timeout_secs": { "type": "integer", "description": "optional formation deadline in seconds; default 0 (unbounded)" },
                    "quorum": { "type": "integer", "description": "K for quorum formation (default ceil(n/2))" }
                },
                "required": [],
            }),
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        if !spawn_nesting_allowed() {
            return Err(format!(
                "spawn nesting depth {} reached max {} (ANGEL_TREEBEARD_MAX_DEPTH)",
                subcall_depth(),
                treebeard_max_depth()
            ));
        }
        let n_cap = env_usize("ANGEL_SPAWN_MAX", SPAWN_MAX_DEFAULT).max(1);
        let mut tasks: Vec<String> = match args.get("tasks").and_then(|t| t.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .filter(|s| !s.trim().is_empty())
                .collect(),
            None => Vec::new(),
        };
        let n = if !tasks.is_empty() {
            tasks.len().min(n_cap)
        } else {
            let task = args
                .get("task")
                .and_then(|t| t.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or("missing 'task' (or 'tasks')")?;
            let n = args
                .get("n")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize)
                .unwrap_or(3)
                .clamp(1, n_cap);
            tasks = vec![task.to_string(); n];
            n
        };
        tasks.truncate(n);
        let formation = Formation::parse(args.get("formation").and_then(|v| v.as_str()), n)?;
        let personas = requested_personas(args.get("persona"))?;
        let grant = match args.get("tools").and_then(|v| v.as_str()) {
            Some(requested) => Grant::parse(Some(requested))?,
            None => self.default_grant(),
        };
        self.enforce_grant_ceiling(grant)?;
        // §3.1.1 / Thm 43-45 (ledger T5): the schedule is *derived* from what each
        // seat claims, not hardcoded on the grant. A `tools=code` seat writes in the
        // one shared workspace, so two of them overlap there and are serialized; a
        // seat that declares a checkout of its own commutes and is admitted at n>1.
        // The refusal names the resource, so a caller can see why.
        let seats: Vec<ChildFootprint> = (1..=n)
            .map(|index| {
                let seat = format!("{}-{index}", grant.label());
                if grant == Grant::Code {
                    ChildFootprint::workspace_writer(seat, self.workspace.clone())
                } else {
                    ChildFootprint::new(seat)
                }
            })
            .collect();
        if let Some(receipt) = independence::refusal(&seats) {
            return Err(format!("tools={} needs n=1 — {receipt}", grant.label()));
        }
        let club_spec = args
            .get("club")
            .and_then(|v| v.as_str())
            .unwrap_or("self")
            .to_string();
        let timeout = configured_formation_timeout(args);
        let quorum_k = args
            .get("quorum")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or_else(|| n.div_ceil(2))
            .clamp(1, n);
        let result = self.run_formation(
            formation, &tasks, &personas, grant, &club_spec, timeout, quorum_k,
        );
        if timeout.is_zero() {
            return result;
        }
        let source = if args.get("timeout_secs").is_some() {
            "spawn.timeout_secs"
        } else {
            "ANGEL_SPAWN_TIMEOUT"
        };
        let note = format!("operator cap {source}={}", timeout.as_secs());
        result
            .map(|text| format!("{text}\n{note}"))
            .map_err(|error| format!("{error}; {note}"))
    }
}

fn configured_formation_timeout(args: &Value) -> Duration {
    Duration::from_secs(
        args.get("timeout_secs")
            .and_then(Value::as_u64)
            .unwrap_or_else(|| env_usize("ANGEL_SPAWN_TIMEOUT", 0) as u64),
    )
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/spawn__l01_tests.rs"]
mod l01_tests;
