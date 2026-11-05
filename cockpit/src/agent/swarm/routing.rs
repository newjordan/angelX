//! Heuristic routing between the single direct pass and the full fan-out.
//!
//! The decisions themselves live in `cockpit/live/swarm-route/swarm-route.bend`:
//! Rust owns the text work (trim, lowercase, word split, phrase windows, prefix
//! tests) and hands the leg a feature record; the leg combines those features
//! into all six decisions at once. The heuristics they replaced answer only as
//! the parity oracle in the test module.

use crate::agent::harness::live_policy;

use crate::platform::native_module::{self, Library, ReloadConfig, ReloadHost};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

type NativeRoute = unsafe extern "C" fn(*const u8, usize, i32, extern "C" fn(u32) -> i32) -> i32;
unsafe extern "C" {
    fn angel_moa_route_v1(
        p: *const u8,
        n: usize,
        mode: i32,
        space: extern "C" fn(u32) -> i32,
    ) -> i32;
}
struct Generation {
    route: NativeRoute,
    _library: Option<Library>,
}
fn baseline() -> Arc<Generation> {
    static BASE: OnceLock<Arc<Generation>> = OnceLock::new();
    Arc::clone(BASE.get_or_init(|| {
        Arc::new(Generation {
            route: angel_moa_route_v1,
            _library: None,
        })
    }))
}
fn load(bytes: &[u8]) -> Result<Arc<Generation>, String> {
    let library = native_module::load(bytes)?;
    let version = library.symbol(c"angel_moa_route_abi_version");
    let route = library.symbol(c"angel_moa_route_v1");
    if version.is_null() || route.is_null() {
        return Err("missing MoA router ABI symbols".into());
    }
    let version: unsafe extern "C" fn() -> u32 = unsafe { std::mem::transmute(version) };
    if unsafe { version() } != 1 {
        return Err("unsupported MoA router ABI".into());
    }
    Ok(Arc::new(Generation {
        route: unsafe { std::mem::transmute(route) },
        _library: Some(library),
    }))
}
fn new_host(path: Option<PathBuf>) -> ReloadHost<Generation> {
    ReloadHost::new(
        path,
        baseline(),
        load,
        ReloadConfig {
            label: "MoA routing",
            thread_name: "angel-moa-routing-reload",
            fallback: "built-in router",
            worker_subject: "MoA routing module",
            missing_is_unavailable: false,
            byte_limit: 16 * 1024 * 1024,
            interval: Duration::from_millis(500),
        },
    )
}
fn host() -> &'static ReloadHost<Generation> {
    static HOST: OnceLock<ReloadHost<Generation>> = OnceLock::new();
    HOST.get_or_init(|| {
        new_host(
            std::env::var_os("ANGEL_MOA_ROUTING_MODULE")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        )
    })
}
extern "C" fn space(value: u32) -> i32 {
    char::from_u32(value).is_some_and(char::is_whitespace) as i32
}
fn run(g: &Generation, problem: &str, mode: i32) -> Option<bool> {
    let value = unsafe { (g.route)(problem.as_ptr(), problem.len(), mode, space) };
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}
fn classify(problem: &str, mode: i32) -> bool {
    classify_with(host(), problem, mode)
}
fn classify_with(host: &ReloadHost<Generation>, problem: &str, mode: i32) -> bool {
    let problem = problem.trim();
    let result = match host {
        ReloadHost::Baseline(g) => run(g, problem, mode),
        _ => run(&host.current(), problem, mode),
    };
    result
        .or_else(|| run(&baseline(), problem, mode))
        .expect("built-in MoA router failed")
}

/// The shipped Bend leg: one evaluation per problem answers every question here.
const LEG: &str = "swarm-route";

/// Trivial conversational turns the swarm can add nothing to.
pub(crate) const TIGHT_PHRASES: &[&str] = &[
    "hi",
    "hello",
    "hey",
    "yo",
    "hiya",
    "thanks",
    "thank you",
    "thx",
    "ty",
    "cheers",
    "ok",
    "okay",
    "k",
    "kk",
    "cool",
    "nice",
    "great",
    "perfect",
    "awesome",
    "got it",
    "gotcha",
    "understood",
    "makes sense",
    "yes",
    "yep",
    "yeah",
    "no",
    "nope",
    "sure",
    "please",
    "continue",
    "go on",
    "go ahead",
    "keep going",
    "proceed",
    "stop",
    "done",
    "good",
    "good job",
    "lgtm",
];

/// What Rust extracted from the problem text: every word/phrase test the
/// heuristics perform, as flags and counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Features {
    blank: bool,
    stripped_blank: bool,
    bytes: u32,
    chars: u32,
    open_hits: u32,
    enum_lines: u32,
    trivial: bool,
    moa_exact: bool,
    moa_prefix: bool,
    open_phrase: bool,
    open_word: bool,
    action_word: bool,
    live_phrase: bool,
    live_word: bool,
}

/// All routing decisions for one problem, as the leg returns them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct Route {
    pub(crate) tight: bool,
    pub(crate) moa: bool,
    pub(crate) synthesis: bool,
    pub(crate) deliberation: bool,
    pub(crate) research: bool,
    pub(crate) hard: bool,
}

impl Features {
    /// The one text pass. Every native heuristic below derives from these same
    /// values, so the leg and the oracle cannot drift apart.
    pub(crate) fn of(problem: &str) -> Self {
        let p = problem.trim().to_ascii_lowercase();
        let stripped = p.trim_matches(|c: char| matches!(c, '.' | '!' | '?' | ',' | ' '));
        let words = route_words(&p);
        Self {
            blank: p.is_empty(),
            stripped_blank: stripped.is_empty(),
            bytes: p.len() as u32,
            chars: p.chars().count() as u32,
            open_hits: DIFFICULTY_WORDS
                .iter()
                .filter(|w| has_route_word(&words, w))
                .count() as u32,
            enum_lines: p.lines().filter(|line| is_enum_line(line)).count() as u32,
            trivial: TIGHT_PHRASES.contains(&stripped),
            moa_exact: p == "moa" || p == "/moa",
            moa_prefix: p.starts_with("moa:") || p.starts_with("/moa "),
            open_phrase: OPEN_PHRASES.iter().any(|m| has_phrase(&p, m)),
            open_word: OPEN_WORDS.iter().any(|m| has_route_word(&words, m)),
            action_word: ACTION_WORDS.iter().any(|m| action_request(&words, m)),
            live_phrase: LIVE_PHRASES.iter().any(|m| p.contains(m)),
            live_word: LIVE_WORDS.iter().any(|m| has_route_word(&words, m)),
        }
    }

    pub(crate) fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "blank": self.blank,
            "stripped_blank": self.stripped_blank,
            "bytes": self.bytes,
            "chars": self.chars,
            "open_hits": self.open_hits,
            "enum_lines": self.enum_lines,
            "trivial": self.trivial,
            "moa_exact": self.moa_exact,
            "moa_prefix": self.moa_prefix,
            "open_phrase": self.open_phrase,
            "open_word": self.open_word,
            "action_word": self.action_word,
            "live_phrase": self.live_phrase,
            "live_word": self.live_word,
        })
    }
}

impl Route {
    /// The contract a loaded generation must satisfy. Synthesis is the narrower
    /// gate — a longer length threshold and a subset of the same word/phrase
    /// tests — so it can never fire without deliberation. An edit that breaks
    /// this keeps the native leg.
    pub(crate) fn check(&self) -> Result<(), String> {
        if self.synthesis && !self.deliberation {
            return Err("a synthesis turn must also earn deliberation".into());
        }
        Ok(())
    }
}

/// Every decision for one problem, in one leg evaluation.
pub(crate) fn route_decisions(problem: &str) -> Route {
    route_with_source(problem).0
}

pub(crate) fn route_with_source(problem: &str) -> (Route, live_policy::LegSource) {
    if std::env::var_os("ANGEL_MOA_ROUTING_MODULE").is_some_and(|path| !path.is_empty()) {
        return (
            Route {
                tight: classify(problem, 0),
                moa: classify(problem, 1),
                synthesis: classify(problem, 2),
                deliberation: classify(problem, 3),
                research: classify(problem, 4),
                hard: classify(problem, 5),
            },
            live_policy::LegSource::Native,
        );
    }
    live_policy::decide_shipped(LEG, &Features::of(problem).json(), Route::check)
}

/// Cheap, conservative pre-classifier: `true` only for trivial conversational
/// turns the swarm can add nothing to (greetings, acknowledgments, simple
/// continuations). These would classify `TIGHT` anyway, so skipping the
/// classifier model call preserves the routing decision while removing one
/// round-trip from the turn — a small but guaranteed latency cut on chit-chat.
/// Anything substantive falls through to the model classifier unchanged.
/// Disable with `ANGEL_SWARM_FASTPATH=0`.
pub(crate) fn obviously_tight(problem: &str) -> bool {
    route_decisions(problem).tight
}

/// Whether the trivial-turn fast path is on (default yes; `ANGEL_SWARM_FASTPATH=0`
/// forces the model classifier on every turn when smart routing is enabled).
pub(crate) fn fastpath_enabled() -> bool {
    std::env::var("ANGEL_SWARM_FASTPATH")
        .map(|v| !matches!(v.trim(), "0" | "false" | "off"))
        .unwrap_or(true)
}

/// Open-ended, reasoning-dominant markers — the questions where fanning out
/// diverse proposers and synthesizing genuinely beats one strong pass. Shared by
/// [`wants_deliberation`] (does the turn deserve the swarm at all) and
/// `wants_synthesis` (should the tool-capable driver's *final answer* be
/// amplified by the MoA fan-out). Kept separate from the action verbs below:
/// an action turn (fix/build/refactor) works through tools and its answer is
/// grounded in real output, so re-synthesizing it text-only can only regress it.
pub(crate) const OPEN_PHRASES: &[&str] = &[
    "trade off",
    "pros and cons",
    "best way",
    "how should",
    "ways to",
    "ideas for",
    "should i",
    "should we",
    "from scratch",
    "what would you",
    "open ended",
    "open-ended",
];
pub(crate) const OPEN_WORDS: &[&str] = &[
    "design",
    "architect",
    "brainstorm",
    "explore",
    "compare",
    "comparison",
    "tradeoff",
    "tradeoffs",
    "options",
    "approaches",
    "alternatives",
    "strategy",
    "strategies",
    "evaluate",
];

pub(crate) const ACTION_WORDS: &[&str] = &[
    "analyze",
    "analyse",
    "investigate",
    "diagnose",
    "debug",
    "fix",
    "repair",
    "harden",
    "build",
    "create",
    "make",
    "implement",
    "add",
    "update",
    "change",
    "modify",
    "refactor",
    "rewrite",
    "write",
    "generate",
    "produce",
    "review",
    "inspect",
    "improve",
    "optimize",
    "plan",
    "wire",
    "route",
    "patch",
    "test",
    "ship",
];

/// Requests that clearly earn the swarm's multi-angle deliberation: open-ended,
/// comparative, exploratory, coding/action requests, or large multi-part asks.
/// Everything else takes the single fast direct pass. Pure heuristic — no model
/// call — so the default router costs **zero** routing round-trips. Anything
/// matched here fans out.
pub(crate) fn wants_deliberation(problem: &str) -> bool {
    route_decisions(problem).deliberation
}

pub(crate) const LIVE_PHRASES: &[&str] = &[
    "most recent",
    "right now",
    "as of today",
    "up to date",
    "up-to-date",
    "this week",
    "this month",
    "current events",
    "current news",
    "current version",
    "latest version",
    "web search",
    "search the web",
    "look up",
    "on x",
    "x.com",
    "twitter",
];
pub(crate) const LIVE_WORDS: &[&str] = &[
    "latest", "recent", "trending", "viral", "breaking", "news", "online", "internet", "live",
];

/// Whether the request needs live outside-world context. This is intentionally
/// narrower than deliberation: it should wake the Grok research scout for
/// latest/current/trending asks, not every ordinary coding or planning turn.
pub(crate) fn wants_live_research(problem: &str) -> bool {
    route_decisions(problem).research
}

pub(crate) fn route_words(problem: &str) -> Vec<&str> {
    problem
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .collect()
}

pub(crate) fn has_route_word(words: &[&str], needle: &str) -> bool {
    words.contains(&needle)
}

pub(crate) fn has_phrase(problem: &str, phrase: &str) -> bool {
    let phrase_words = route_words(phrase);
    route_words(problem)
        .windows(phrase_words.len())
        .any(|w| w.iter().copied().eq(phrase_words.iter().copied()))
}

pub(crate) fn action_request(words: &[&str], action: &str) -> bool {
    let Some(idx) = words.iter().position(|w| *w == action) else {
        return false;
    };
    if idx == 0 {
        return true;
    }
    let prefix = &words[..idx];
    matches!(
        prefix,
        ["please"]
            | ["pls"]
            | ["can", "you"]
            | ["could", "you"]
            | ["would", "you"]
            | ["will", "you"]
            | ["i", "want", "you", "to"]
            | ["i", "need", "you", "to"]
            | ["we", "need", "to"]
            | ["need", "to"]
            | ["let's"]
            | ["lets"]
            | ["have", "it"]
            | ["have", "the", "agent"]
    )
}

/// Default router is SPEED: answer directly unless [`wants_deliberation`]. Set
/// `ANGEL_SWARM_SMART_ROUTE=1` to instead ask the model to classify each turn —
/// adaptive, but one extra round-trip per turn.
pub(crate) fn smart_route_enabled() -> bool {
    std::env::var("ANGEL_SWARM_SMART_ROUTE")
        .map(|v| matches!(v.trim(), "1" | "true" | "on"))
        .unwrap_or(false)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Difficulty {
    Normal,
    Hard,
}

pub(crate) const DIFFICULTY_WORDS: &[&str] = &[
    "design",
    "architect",
    "compare",
    "tradeoffs",
    "strategy",
    "plan",
    "implement",
    "debug",
    "refactor",
    "evaluate",
    "optimize",
];

pub(crate) fn is_enum_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("- ")
        || t.starts_with("* ")
        || t.chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() && t.get(1..2) == Some("."))
}

pub(crate) fn estimate_difficulty(problem: &str) -> Difficulty {
    if route_decisions(problem).hard {
        Difficulty::Hard
    } else {
        Difficulty::Normal
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/swarm_route_module__tests.rs"]
mod native_module_tests;
