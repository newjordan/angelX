//! Heuristic routing between the single direct pass and the full fan-out.

/// Cheap, conservative pre-classifier: `true` only for trivial conversational
/// turns the swarm can add nothing to (greetings, acknowledgments, simple
/// continuations). These would classify `TIGHT` anyway, so skipping the
/// classifier model call preserves the routing decision while removing one
/// round-trip from the turn — a small but guaranteed latency cut on chit-chat.
/// Anything substantive falls through to the model classifier unchanged.
/// Disable with `ANGEL_SWARM_FASTPATH=0`.
pub(crate) fn obviously_tight(problem: &str) -> bool {
    let p = problem.trim().to_ascii_lowercase();
    let p = p.trim_matches(|c: char| matches!(c, '.' | '!' | '?' | ',' | ' '));
    matches!(
        p,
        "" | "hi"
            | "hello"
            | "hey"
            | "yo"
            | "hiya"
            | "thanks"
            | "thank you"
            | "thx"
            | "ty"
            | "cheers"
            | "ok"
            | "okay"
            | "k"
            | "kk"
            | "cool"
            | "nice"
            | "great"
            | "perfect"
            | "awesome"
            | "got it"
            | "gotcha"
            | "understood"
            | "makes sense"
            | "yes"
            | "yep"
            | "yeah"
            | "no"
            | "nope"
            | "sure"
            | "please"
            | "continue"
            | "go on"
            | "go ahead"
            | "keep going"
            | "proceed"
            | "stop"
            | "done"
            | "good"
            | "good job"
            | "lgtm"
    )
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
/// [`wants_synthesis`] (should the tool-capable driver's *final answer* be
/// amplified by the MoA fan-out). Kept separate from the action verbs below:
/// an action turn (fix/build/refactor) works through tools and its answer is
/// grounded in real output, so re-synthesizing it text-only can only regress it.
const OPEN_PHRASES: &[&str] = &[
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
const OPEN_WORDS: &[&str] = &[
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

/// Explicit user summon for the full mixture: the turn opens with `moa:` (or
/// `/moa`). In `MoaMode::Manual` — the sota-moa default — this is the ONLY way
/// a turn earns amplification; every other turn runs coordinator-only at
/// standard token spend. Leading-position only, so prose that merely mentions
/// "moa" never triggers a fan-out.
pub(crate) fn summons_moa(problem: &str) -> bool {
    let p = problem.trim().to_ascii_lowercase();
    p == "moa" || p == "/moa" || p.starts_with("moa:") || p.starts_with("/moa ")
}

/// Open-ended reasoning: the request is asking for judgment/synthesis, not a
/// mechanical action. This is the gate for amplifying a tool-capable driver's
/// final answer with the MoA fan-out — reserved for the questions multi-agent
/// deliberation actually improves, so ordinary coding turns pay zero MoA cost.
pub(crate) fn wants_synthesis(problem: &str) -> bool {
    let p = problem.trim().to_ascii_lowercase();
    if p.is_empty() {
        return false;
    }
    if p.len() > 600 {
        return true; // a long, multi-part brief benefits from synthesis
    }
    if OPEN_PHRASES.iter().any(|m| has_phrase(&p, m)) {
        return true;
    }
    let words = route_words(&p);
    OPEN_WORDS.iter().any(|m| has_route_word(&words, m))
}

/// Requests that clearly earn the swarm's multi-angle deliberation: open-ended,
/// comparative, exploratory, coding/action requests, or large multi-part asks.
/// Everything else takes the single fast direct pass. Pure heuristic — no model
/// call — so the default router costs **zero** routing round-trips. Anything
/// matched here fans out.
pub(crate) fn wants_deliberation(problem: &str) -> bool {
    let p = problem.trim().to_ascii_lowercase();
    if p.is_empty() {
        return false;
    }
    if p.len() > 400 {
        return true; // long / multi-part → diverse proposers help
    }
    let words = route_words(&p);
    if OPEN_PHRASES.iter().any(|m| has_phrase(&p, m)) {
        return true;
    }
    if OPEN_WORDS.iter().any(|m| has_route_word(&words, m)) {
        return true;
    }
    const ACTION_WORDS: &[&str] = &[
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
    ACTION_WORDS.iter().any(|m| action_request(&words, m))
}

/// Whether the request needs live outside-world context. This is intentionally
/// narrower than deliberation: it should wake the Grok research scout for
/// latest/current/trending asks, not every ordinary coding or planning turn.
pub(crate) fn wants_live_research(problem: &str) -> bool {
    let p = problem.trim().to_ascii_lowercase();
    if p.is_empty() {
        return false;
    }
    const LIVE_PHRASES: &[&str] = &[
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
    if LIVE_PHRASES.iter().any(|m| p.contains(m)) {
        return true;
    }
    let words = route_words(&p);
    const LIVE_WORDS: &[&str] = &[
        "latest", "recent", "trending", "viral", "breaking", "news", "online", "internet", "live",
    ];
    LIVE_WORDS.iter().any(|m| has_route_word(&words, m))
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

pub(crate) fn estimate_difficulty(problem: &str) -> Difficulty {
    let p = problem.trim().to_ascii_lowercase();
    if p.chars().count() > 800 {
        return Difficulty::Hard;
    }
    let words = route_words(&p);
    let open_hits = [
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
    ]
    .iter()
    .filter(|w| has_route_word(&words, w))
    .count();
    if open_hits >= 2 {
        return Difficulty::Hard;
    }
    let enum_lines = p
        .lines()
        .filter(|line| {
            let t = line.trim_start();
            t.starts_with("- ")
                || t.starts_with("* ")
                || t.chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit() && t.get(1..2) == Some("."))
        })
        .count();
    if enum_lines >= 3 {
        Difficulty::Hard
    } else {
        Difficulty::Normal
    }
}
