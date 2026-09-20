//! The quest thinking map — a reasoning trace drawn as an adventure.
//!
//! Feed it a model's raw chain-of-thought and it charts the *pivots* — every
//! "But…", every "Wait, actually…", every place the model rejects its own
//! idea — as a tree of expeditions: adventurers set out from a crossroads,
//! some fall where the pivot felled them, one (maybe) reaches the goal. Ask
//! several models the same question and `render_party` chronicles the whole
//! gauntlet, one themed map per model.
//!
//! Like the miniworld (`world_viz.rs`) this is **pure display**: nothing here
//! feeds back into the model, the turn, or any decision logic. And it is
//! **deterministic**: the same trace + label renders the same map forever —
//! themes and adventurer names derive from an FNV hash of the label; no
//! clocks, no RNG.
//!
//! The v0 fork model is deliberately simple: a rejection/reversal fells the
//! current expedition and the next one departs from the last deliberate
//! crossroads (the most recent "Alternatively…"-style fork, or the quest
//! start). Smarter fork-point inference is a handoff milestone
//! (`docs/questmap/QUESTMAP_HANDOFF.md`).

/// Hard cap on parsed steps — keeps a runaway trace from flooding the
/// transcript. The footer notes when the chronicle is truncated.
const MAX_STEPS: usize = 400;
/// Hard cap on rendered expedition lines (the tree can be wider than the map).
const MAX_EXPEDITIONS_SHOWN: usize = 24;
/// Hard cap on annotated lines in the lexicon view (`/quest lex`).
const MAX_LEX_SHOWN: usize = 60;
/// Snippet length (chars) quoted from a step.
const SNIP: usize = 56;

/// What a single reasoning step *is*, judged by how it opens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum StepKind {
    /// Plain forward progress.
    Step,
    /// Self-questioning that doesn't abandon the path ("Hmm", "let me check").
    Doubt,
    /// A deliberate fork ("Alternatively…") — the old path still stands.
    Alternative,
    /// The model shoots down its own idea ("But that ignores…").
    Rejection,
    /// A full turnabout ("Wait, actually…").
    Reversal,
    /// An answer is claimed ("So the answer is…").
    Conclusion,
}

/// Markers are matched against the lowercased *start* of a step, most decisive
/// class first (a "Wait, but…" is a reversal, not a rejection). Tuning this
/// lexicon against real fleet traces is milestone M2 in the handoff.
const REVERSAL: &[&str] = &[
    "wait",
    "no wait",
    "no, wait",
    "hold on",
    "hang on",
    "actually",
    "oh wait",
    "oh no",
    "on second thought",
    "scratch that",
    "hmm, no",
    "no, that",
    "i take that back",
    "let me reconsider",
    "let me rethink",
    "i was wrong",
];
const CONCLUSION: &[&str] = &[
    "so the answer",
    "so, the answer",
    "the answer is",
    // "Answer: 258 rolls." — a very common conclusion phrasing real models use
    // (found tuning against deepseek traces, M2); the "the answer is" entry
    // above missed the bare form.
    "answer:",
    "answer is",
    "therefore",
    "thus",
    "hence",
    "final answer",
    "in conclusion",
    "conclusion:",
    "so the probability",
];
const REJECTION: &[&str] = &[
    "but ",
    "but,",
    "but—",
    "however",
    "the problem is",
    "the issue is",
    "that doesn't",
    "this doesn't",
    "that can't",
    "this can't",
    "that's not right",
    "that's wrong",
    "this is wrong",
    "that contradicts",
    "that fails",
    "this fails",
    "except",
    "unfortunately",
];
const ALTERNATIVE: &[&str] = &[
    "alternatively",
    "or maybe",
    "or perhaps",
    "another approach",
    "another way",
    "let me try",
    "let's try",
    "what if",
    "instead",
    "a different",
    "or we could",
    "suppose instead",
    "second approach",
    "plan b",
];
const DOUBT: &[&str] = &[
    "hmm",
    "hm.",
    "hm,",
    "i'm not sure",
    "im not sure",
    "not sure",
    "is that right",
    "is this right",
    "let me check",
    "let me verify",
    "let me double",
    "let me sanity",
    "let me make sure",
    "double-check",
    "double check",
    "sanity check",
];

/// One parsed reasoning step, hung in the expedition tree.
struct QuestNode {
    parent: Option<usize>,
    kind: StepKind,
    text: String,
    /// This node sits on a path the model later walked away from.
    abandoned: bool,
    /// Set on the tip a pivot felled: the pivot's kind + quoted snippet.
    fell: Option<(StepKind, String)>,
}

/// The parsed trace: a tree of steps rooted at the quest's opening line.
pub(crate) struct QuestTree {
    nodes: Vec<QuestNode>,
    /// Where the trace ended — the would-be victor's tip.
    tip: usize,
    truncated: bool,
}

/// A rendering skin. All glyphs are single-column so the map survives any
/// terminal; rosters are 8 wide to match "ask 8 models one question".
pub(crate) struct Theme {
    pub(crate) name: &'static str,
    arena: &'static str,
    party: [&'static str; 8],
    step_one: &'static str,
    step_many: &'static str,
    start_glyph: char,
    fall_glyph: char,
    fall_verb: &'static str,
    turn_verb: &'static str,
    win_glyph: char,
    win_phrase: &'static str,
    wander_glyph: char,
    wander_phrase: &'static str,
    outcome_won: &'static str,
    outcome_lost: &'static str,
}

/// The theme roster. Variety is the point: which skin a model gets is a
/// stable hash of its label, so Sir Percival's map and the Kestrel's chart
/// can chronicle the same question side by side.
pub(crate) const THEMES: &[Theme] = &[
    Theme {
        name: "overworld",
        arena: crate::identity::QUEST_ARENA_OVERWORLD,
        party: crate::identity::QUEST_PARTY_OVERWORLD,
        step_one: crate::identity::QUEST_LEAGUE,
        step_many: crate::identity::QUEST_LEAGUES,
        start_glyph: '⚐',
        fall_glyph: '☠',
        fall_verb: crate::identity::QUEST_FALLS,
        turn_verb: crate::identity::QUEST_TURNS_BACK,
        win_glyph: '♜',
        win_phrase: crate::identity::QUEST_REACHES_CASTLE,
        wander_glyph: '~',
        wander_phrase: crate::identity::QUEST_OVERWORLD_WANDERING,
        outcome_won: crate::identity::QUEST_CASTLE_WON,
        outcome_lost: crate::identity::QUEST_CASTLE_UNTAKEN,
    },
    Theme {
        name: "dungeon",
        arena: crate::identity::QUEST_ARENA_DUNGEON,
        party: crate::identity::QUEST_PARTY_DUNGEON,
        step_one: crate::identity::QUEST_CHAMBER,
        step_many: crate::identity::QUEST_CHAMBERS,
        start_glyph: '▼',
        fall_glyph: '✗',
        fall_verb: crate::identity::QUEST_HITS_DEAD_END,
        turn_verb: crate::identity::QUEST_DOUBLES_BACK,
        win_glyph: '◆',
        win_phrase: crate::identity::QUEST_LIFTS_TREASURE,
        wander_glyph: '·',
        wander_phrase: crate::identity::QUEST_TORCH_LIT,
        outcome_won: crate::identity::QUEST_TREASURE_CLAIMED,
        outcome_lost: crate::identity::QUEST_VAULT_SEALED,
    },
    Theme {
        name: "voyage",
        arena: crate::identity::QUEST_ARENA_VOYAGE,
        party: crate::identity::QUEST_PARTY_VOYAGE,
        step_one: crate::identity::QUEST_DAY_OUT,
        step_many: crate::identity::QUEST_DAYS_OUT,
        start_glyph: '⚓',
        fall_glyph: '✘',
        fall_verb: crate::identity::QUEST_WRECKS,
        turn_verb: crate::identity::QUEST_COMES_ABOUT,
        win_glyph: '⚑',
        win_phrase: crate::identity::QUEST_MAKES_PORT,
        wander_glyph: '~',
        wander_phrase: crate::identity::QUEST_STILL_AT_SEA,
        outcome_won: crate::identity::QUEST_LANDFALL,
        outcome_lost: crate::identity::QUEST_NO_LANDFALL,
    },
];

/// FNV-1a — the module's only "randomness"; stable across runs by design.
fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The stable theme for a model label (override wins if it names one).
pub(crate) fn theme_for(label: &str, override_name: Option<&str>) -> &'static Theme {
    if let Some(want) = override_name
        && let Some(t) = THEMES.iter().find(|t| t.name.eq_ignore_ascii_case(want))
    {
        return t;
    }
    #[allow(clippy::cast_possible_truncation)]
    &THEMES[(fnv(label) % THEMES.len() as u64) as usize]
}

/// Split a trace into steps: sentence enders followed by whitespace, and
/// newlines. Decimals ("13/27, 0.5") survive because '.' only splits before
/// whitespace.
fn segments(trace: &str) -> (Vec<String>, bool) {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = trace.chars().peekable();
    let mut truncated = false;
    while let Some(c) = chars.next() {
        if out.len() >= MAX_STEPS {
            truncated = true;
            break;
        }
        if c == '\n' {
            flush(&mut cur, &mut out);
            continue;
        }
        cur.push(c);
        if matches!(c, '.' | '?' | '!') && chars.peek().is_none_or(|n| n.is_whitespace()) {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    (out, truncated)
}

fn flush(cur: &mut String, out: &mut Vec<String>) {
    let t = cur.trim();
    if t.chars().count() > 2 {
        out.push(t.to_string());
    }
    cur.clear();
}

/// The marker lexicon in precedence order — the first class whose markers match
/// the lowercased *start* of a step wins ("Wait, but…" is a Reversal, not a
/// Rejection). Written from priors in M0; tuning it against real fleet traces is
/// milestone M2. A single ordered table (rather than a chain of `if`s) makes the
/// precedence explicit and lets `classify_explain` report which marker fired, so
/// a real trace can be eyeballed line by line (`/quest lex`).
const LEXICON: &[(StepKind, &[&str])] = &[
    (StepKind::Reversal, REVERSAL),
    (StepKind::Conclusion, CONCLUSION),
    (StepKind::Rejection, REJECTION),
    (StepKind::Alternative, ALTERNATIVE),
    (StepKind::Doubt, DOUBT),
];

/// Which class of step this is, by its opening words.
fn classify(seg: &str) -> StepKind {
    classify_explain(seg).0
}

/// Strip a leading run of markdown / list / emphasis punctuation ("**", "- ",
/// "#", "> ", "• ") so a marker at the *semantic* start of a line still matches.
/// Real model reasoning is markdown-formatted — "**Answer: …**", "- But that…" —
/// and without this the emphasis defeats `starts_with` and the marker is missed
/// (found tuning against real deepseek traces, M2). Classification-only: the
/// text shown in the map/lexicon view is never altered.
fn strip_lead(seg: &str) -> &str {
    seg.trim_start()
        .trim_start_matches(['*', '_', '`', '#', '>', '-', '•', '·', ' ', '\t'])
}

/// Classify a step *and* report which marker matched — the lexicon-tuning
/// instrument behind `/quest lex`. A `None` marker means nothing matched and it
/// fell through to a plain [`StepKind::Step`]. Pure, like `classify`.
fn classify_explain(seg: &str) -> (StepKind, Option<&'static str>) {
    let s = strip_lead(seg).to_ascii_lowercase();
    for (kind, markers) in LEXICON {
        if let Some(m) = markers.iter().find(|m| s.starts_with(**m)) {
            return (*kind, Some(m));
        }
    }
    (StepKind::Step, None)
}

/// A short tag for a step class — used by the lexicon view's annotations.
fn kind_tag(kind: StepKind) -> &'static str {
    match kind {
        StepKind::Step => "step",
        StepKind::Doubt => "doubt",
        StepKind::Alternative => "fork",
        StepKind::Rejection => "reject",
        StepKind::Reversal => "reverse",
        StepKind::Conclusion => "answer",
    }
}

/// First `n` chars, ellipsized, double quotes softened so the map's own
/// quoting stays readable.
fn snip(s: &str, n: usize) -> String {
    let clean: String = s.chars().map(|c| if c == '"' { '\'' } else { c }).collect();
    if clean.chars().count() <= n {
        clean
    } else {
        let mut cut: String = clean.chars().take(n.saturating_sub(1)).collect();
        cut.push('…');
        cut
    }
}

fn push_node(nodes: &mut Vec<QuestNode>, parent: usize, kind: StepKind, text: &str) -> usize {
    nodes.push(QuestNode {
        parent: Some(parent),
        kind,
        text: text.to_string(),
        abandoned: false,
        fell: None,
    });
    nodes.len() - 1
}

/// Parse a raw reasoning trace into the expedition tree.
pub(crate) fn parse(trace: &str) -> QuestTree {
    let (segs, truncated) = segments(trace);
    let mut iter = segs.into_iter();
    let root_text = iter.next().unwrap_or_else(|| "(a blank page)".to_string());
    let mut nodes = vec![QuestNode {
        parent: None,
        kind: StepKind::Step,
        text: root_text,
        abandoned: false,
        fell: None,
    }];
    let mut cur = 0_usize;
    // The last deliberate crossroads — where a felled expedition's successor
    // departs from.
    let mut fork = 0_usize;
    for seg in iter {
        let kind = classify(&seg);
        match kind {
            StepKind::Rejection | StepKind::Reversal if cur != fork => {
                nodes[cur].fell = Some((kind, snip(&seg, SNIP)));
                let mut walk = cur;
                while walk != fork {
                    nodes[walk].abandoned = true;
                    walk = nodes[walk].parent.unwrap_or(fork);
                }
                cur = push_node(&mut nodes, fork, kind, &seg);
            }
            // A pivot with nothing yet to reject reads as a plain step out.
            StepKind::Alternative => {
                fork = cur;
                cur = push_node(&mut nodes, cur, kind, &seg);
            }
            _ => {
                cur = push_node(&mut nodes, cur, kind, &seg);
            }
        }
    }
    QuestTree {
        nodes,
        tip: cur,
        truncated,
    }
}

impl QuestTree {
    /// Children lists, index-aligned with `nodes`.
    fn children(&self) -> Vec<Vec<usize>> {
        let mut kids = vec![Vec::new(); self.nodes.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            if let Some(p) = n.parent {
                kids[p].push(i);
            }
        }
        kids
    }

    /// The node set on the path root→tip (the surviving line of reasoning).
    fn victor_path(&self) -> Vec<usize> {
        let mut path = Vec::new();
        let mut walk = Some(self.tip);
        while let Some(i) = walk {
            path.push(i);
            walk = self.nodes[i].parent;
        }
        path
    }

    /// Did the surviving path actually claim an answer?
    fn resolved(&self) -> bool {
        self.victor_path()
            .iter()
            .any(|&i| self.nodes[i].kind == StepKind::Conclusion)
    }
}

/// Render one model's trace as a themed quest map. `width` clamps every line
/// (transcript panes are narrow; 72 is a good default).
pub(crate) fn render(tree: &QuestTree, theme: &Theme, label: &str, width: usize) -> String {
    let kids = tree.children();
    // Expeditions = leaves, chronicled in DFS order.
    let mut leaves = Vec::new();
    let mut stack = vec![0_usize];
    while let Some(i) = stack.pop() {
        if kids[i].is_empty() {
            leaves.push(i);
        } else {
            for &k in kids[i].iter().rev() {
                stack.push(k);
            }
        }
    }
    let victor: Vec<usize> = tree.victor_path();
    let resolved = tree.resolved();

    let mut out = Vec::new();
    out.push(format!(
        "{} QUEST MAP · {} · {label}",
        theme.start_glyph, theme.arena
    ));
    out.push(format!(
        "  the quest: \"{}\"",
        snip(&tree.nodes[0].text, SNIP)
    ));
    let shown = leaves.len().min(MAX_EXPEDITIONS_SHOWN);
    for (idx, &leaf) in leaves.iter().take(shown).enumerate() {
        // Expedition length: nodes from its nearest fork (exclusive) to the leaf.
        let mut len = 0_usize;
        let mut depth = 0_usize;
        let mut walk = leaf;
        let mut counting = true;
        while let Some(p) = tree.nodes[walk].parent {
            if kids[p].len() > 1 {
                counting = false;
                depth += 1;
            }
            if counting {
                len += 1;
            }
            walk = p;
        }
        len = len.max(1);
        let indent = "│ ".repeat(depth.saturating_sub(1));
        let joint = if idx + 1 == shown { '└' } else { '├' };
        let name = roster_name(theme, idx);
        let steps = if len == 1 {
            theme.step_one
        } else {
            theme.step_many
        };
        let line = if victor.contains(&leaf) && resolved {
            format!(
                "{indent}{joint}─{} {name} — {len} {steps}, {}",
                theme.win_glyph, theme.win_phrase
            )
        } else if let Some((kind, at)) = &tree.nodes[leaf].fell {
            let verb = if *kind == StepKind::Reversal {
                theme.turn_verb
            } else {
                theme.fall_verb
            };
            format!(
                "{indent}{joint}─{} {name} — {len} {steps}, {verb}: \"{at}\"",
                theme.fall_glyph
            )
        } else {
            format!(
                "{indent}{joint}─{} {name} — {len} {steps}, {}",
                theme.wander_glyph, theme.wander_phrase
            )
        };
        out.push(line);
        if victor.contains(&leaf) && resolved {
            let last_claim = victor
                .iter()
                .find(|&&i| tree.nodes[i].kind == StepKind::Conclusion)
                .map_or_else(
                    || tree.nodes[tree.tip].text.clone(),
                    |&i| tree.nodes[i].text.clone(),
                );
            out.push(format!("{indent}    ⇒ \"{}\"", snip(&last_claim, SNIP)));
        }
    }
    if leaves.len() > shown {
        out.push(format!("  … and {} more expeditions", leaves.len() - shown));
    }
    let felled = leaves
        .iter()
        .filter(|&&l| tree.nodes[l].fell.is_some())
        .count();
    let reversals = tree
        .nodes
        .iter()
        .filter(|n| n.kind == StepKind::Reversal)
        .count();
    let rejections = tree
        .nodes
        .iter()
        .filter(|n| n.kind == StepKind::Rejection)
        .count();
    let outcome = if resolved {
        theme.outcome_won
    } else {
        theme.outcome_lost
    };
    let trunc = if tree.truncated {
        " · chronicle truncated"
    } else {
        ""
    };
    let pivots = reversals + rejections;
    out.push(format!(
        "  {} expedition{} · {pivots} pivot{} ({felled} felled) · {outcome}{trunc}",
        leaves.len(),
        if leaves.len() == 1 { "" } else { "s" },
        if pivots == 1 { "" } else { "s" },
    ));
    out.iter()
        .map(|l| clamp_line(l, width))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Roster names repeat with a generation mark past 8 ("Sir Kay II").
fn roster_name(theme: &Theme, idx: usize) -> String {
    let generation = idx / theme.party.len();
    let base = theme.party[idx % theme.party.len()];
    match generation {
        0 => base.to_string(),
        1 => format!("{base} II"),
        _ => format!("{base} {}", generation + 1),
    }
}

fn clamp_line(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_string()
    } else {
        let mut cut: String = s.chars().take(width.saturating_sub(1)).collect();
        cut.push('…');
        cut
    }
}

/// One-call convenience for `/quest`: parse + theme + render.
pub(crate) fn render_trace(
    label: &str,
    trace: &str,
    theme_override: Option<&str>,
    width: usize,
) -> String {
    if trace.trim().is_empty() {
        return "the map is blank — no reasoning trace to chart".to_string();
    }
    let tree = parse(trace);
    render(&tree, theme_for(label, theme_override), label, width)
}

/// The lexicon view (`/quest lex`): every step of a trace with the class it was
/// assigned and the marker that fired. Where the quest map shows the *shape* of
/// the reasoning, this shows *why* each step was read as it was — the M2 tuning
/// instrument, so the marker tables can be adjusted against real fleet traces.
/// Pure display, single-column, bounded like the map.
pub(crate) fn render_lexicon(label: &str, trace: &str, width: usize) -> String {
    if trace.trim().is_empty() {
        return "the lexicon has nothing to read — no reasoning trace".to_string();
    }
    let (segs, truncated) = segments(trace);
    let classified: Vec<(StepKind, Option<&'static str>)> =
        segs.iter().map(|s| classify_explain(s)).collect();

    let mut out = vec![format!("⚙ LEXICON · {label}")];
    let shown = segs.len().min(MAX_LEX_SHOWN);
    for i in 0..shown {
        let (kind, marker) = classified[i];
        let tag = kind_tag(kind);
        // Show the marker that fired for every non-plain class — that's the
        // whole point of the view when tuning the tables.
        let note = match marker {
            Some(m) if kind != StepKind::Step => format!("{tag} ← \"{m}\""),
            _ => tag.to_string(),
        };
        out.push(clamp_line(
            &format!("{:>3} [{note}] {}", i + 1, snip(&segs[i], SNIP)),
            width,
        ));
    }
    if segs.len() > shown {
        out.push(format!("  … and {} more steps", segs.len() - shown));
    }
    // Footer tally over the whole trace — non-plain classes, in precedence order.
    let order = [
        StepKind::Reversal,
        StepKind::Conclusion,
        StepKind::Rejection,
        StepKind::Alternative,
        StepKind::Doubt,
    ];
    let tally = order
        .iter()
        .filter_map(|k| {
            let n = classified.iter().filter(|(c, _)| c == k).count();
            (n > 0).then(|| format!("{} {n}", kind_tag(*k)))
        })
        .collect::<Vec<_>>()
        .join(" · ");
    let trunc = if truncated { " · truncated" } else { "" };
    out.push(clamp_line(
        &format!(
            "  {} steps · {}{trunc}",
            segs.len(),
            if tally.is_empty() {
                "all plain steps".to_string()
            } else {
                tally
            },
        ),
        width,
    ));
    out.join("\n")
}

/// Chronicle several models given the same question — one themed map each.
/// The gauntlet runner (`/quest gauntlet`) feeds this the reasoning it tapped
/// from each club; the tests exercise it directly.
pub(crate) fn render_party(traces: &[(String, String)], width: usize) -> String {
    let mut out = vec![format!(
        "⚔ THE GAUNTLET · {} chroniclers, one quest",
        traces.len()
    )];
    for (label, trace) in traces {
        out.push(String::new());
        out.push(render_trace(label, trace, None, width));
    }
    out.join("\n")
}

/// A built-in sample trace (`/quest sample`) — the boy-born-on-Tuesday problem,
/// with the classic wrong turn. Fixed example text so the map can be exercised
/// on the offline practice agent, which produces no reasoning of its own; it is
/// never presented as the operator's own reasoning.
pub(crate) const SAMPLE_TRACE: &str = "\
A family has two children and at least one is a boy born on a Tuesday; what \
is the probability both are boys?
My first instinct is that the day can't matter, so this is the classic \
two-child problem.
The condition 'at least one boy' leaves BB, BG, GB equally likely, so the \
probability is 1/3.
But the Tuesday detail must matter or it wouldn't be in the problem.
Let me enumerate: each child is one of 14 equally likely sex-day pairs.
Pairs with at least one Tuesday-boy: 14 + 14 - 1 = 27.
Hmm, let me double-check that subtraction.
The both-Tuesday-boys pair is counted twice, so 27 is right.
Of those, the both-boys cases: 7 + 7 - 1 = 13.
Let me sanity check against the classic version: as the extra condition gets \
rarer the answer should drift toward 1/2, and 13/27 sits just under it.
So the answer is 13/27.";

#[cfg(test)]
#[path = "../../tests/cockpit/app/questmap__tests.rs"]
mod tests;
