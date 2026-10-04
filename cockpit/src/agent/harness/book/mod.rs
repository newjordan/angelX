//! The book of behaviors: a braille routing tree, the harness's only voice to
//! the model.
//!
//! Table of contents — the first layer. Each primary is one braille cell and
//! one chapter file; its sections attach to it as braille digits (⠁1 ⠃2 ⠉3
//! ⠙4 ⠑5 ⠋6 ⠛7 ⠓8 ⠊9 ⠚0), and a section's pages attach to it as a second
//! digit. A primary is any cell that is neither a digit nor a tray glyph (⠿
//! ⠤); the lowered cells (⠂ ⠆ ⠒ ⠲ ⠢ ⠖ ⠶ ⠦ ⠔ ⠴) read as digits and stay
//! unused. The book grows in volumes:
//!
//! - **Volume I, the letters k–z:** the core behaviors, below.
//! - **Volume II, the contractions ⠡ ch ⠩ sh ⠹ th ⠱ wh ⠫ ed ⠣ gh ⠷ of ⠾ with
//!   ⠯ and ⠮ the:** the tool library — how each tool is meant to be used and
//!   what each of its parameters means, off its schema.
//! - **Volume III, the next contractions:** the drivers and their seats.
//! - **Volume IV, the cells without a contraction ⠄ ⠈ ⠐:** the mixture — the
//!   mixture-of-agents stages, their angles and their frames.
//! - **Volume V, the long run, ⠘ ⠠:** what a `/loop` iteration is told, and
//!   the long run's seats.
//! - **Volume VI, the cells ⠨ ⠰ and the overflow shelf ⡨:** the replies — tool
//!   results that carry the way back, and the long reply bodies.
//! - **Volume VII, the seats and the preamble ⠸ ⡸:** the seats' briefs and the
//!   pinned preamble's frames; ⡸ is the 8-dot shelf (dot 7 on ⠸).
//! - **Volume VIII, the hop advisories ⠼ and the competition's shelf ⡅:** what
//!   the 0.1.6 harness said to a model at the moments that decide a run, in the
//!   words it said them (see [`VOICED`]).
//! - **Volume IX, the seat profiles ⡞ ⢞ ⣞ ⡯:** what a model's own vendor
//!   harness tells it, in its words. Every 6-dot cell is taken, so the volume
//!   lives on 8-dot shelves: ⡞ ⢞ ⣞ (dots 7, 8 and 7+8 on ⠞) and ⡯ (dot 7 on ⠯).
//!   A library: nothing in it is sent to a seat until a driver is wired to it,
//!   and one page is: an OpenAI seat stands on ⡞⠙⠓.
//!
//! Volume I:
//!
//! | cell | chapter                | surface                                              |
//! |------|------------------------|------------------------------------------------------|
//! | ⠅ k  | `k_competition.rs`     | the competition loop and its techniques              |
//! | ⠇ l  | `l_loops.rs`           | work came back with the same outcome                 |
//! | ⠍ m  | `m_method.rs`          | task pace, coding repair discipline, map/recon frames, Treebeard lane |
//! | ⠝ n  | `n_environment.rs`     | verified capabilities, the interactive cockpit, vision |
//! | ⠕ o  | `o_orchestration.rs`   | delegate routes, delegate, spawn, swarm_compile      |
//! | ⠏ p  | `p_processes.rs`       | background jobs                                      |
//! | ⠟ q  | `q_stop.rs`            | the stop checkpoint and the if-stuck strategies      |
//! | ⠗ r  | `r_relentless.rs`      | operator-armed relentless execution                  |
//! | ⠎ s  | `s_sources.rs`         | the research (sources) contract                      |
//! | ⠞ t  | `t_personality.rs`     | the Driver's identity and standing posture           |
//! | ⠥ u  | `u_skills.rs`          | playbook catalog, referral card and playbook hint    |
//! | ⠧ v  | `v_verification.rs`    | what the tests and checks say; the verification posture |
//! | ⠺ w  | `w_workflow.rs`        | tool protocol, batching, response, promise; timed cues |
//! | ⠭ x  | `x_execution.rs`       | calls and replies that did not execute               |
//! | ⠽ y  | `y_types.rs`           | personality types: bundles of sections               |
//! | ⠵ z  | `z_brevity.rs`         | caveman brevity: the rules and their levels          |
//!
//! Volume II, the tool library — one section per tool, named for it; a tool's
//! schema ends with its route, and each parameter's description is the address
//! of its page (`⠣⠉⠃`), or consecutive addresses for a description of several
//! sentences. A tool with more than ten pages continues in a section named
//! `<tool>, continued`; a schema whose guidance runs there names both routes. A
//! page that keeps a `{placeholder}` has its value ride after the address as
//! data (`⠱⠁⠃ http://…`), after the route when it is the description's:
//!
//! | cell | chapter       | tools                                                          |
//! |------|---------------|----------------------------------------------------------------|
//! | ⠡ ch | `ch_edits.rs` | read_file, str_replace, git_diff, git_log, file_search, semantic_read, lsp_diagnostics, notes, handoff, recall |
//! | ⠩ sh | `sh_shell.rs` | shell, cargo, lint, proc_run, proc_status, proc_wait, tool_repair, loop_research, rl_campaign, benchmark_compare |
//! | ⠹ th | `th_team.rs`  | delegate, swarm_compile, consult_model, agent_graph, spawn, code_mode, handle_read, tool_search, skill, jev_decide |
//! | ⠱ wh | `wh_world.rs` | web_search, web_fetch, science_search, repo_search, grok_research, http_request, fleet_status, vast_instances, llm_probe, llm_bench |
//! | ⠫ ed | `ed_media.rs` | present, video_probe, video_beats, video_cut, video_contact_sheet, video_look, vision_look, knowledge_graph, continual_harness, work_landing |
//! | ⠣ gh | `gh_navigation.rs` | outline, list_dir, grep, find_files, defs, git_status, self_map |
//! | ⠷ of | `of_patches.rs` | write_file, multi_edit, apply_patch, resolve_edit, git_commit, integrate |
//! | ⠾ with | `with_build.rs` | run_tests, check, fmt, proc_stop; shell, loop_research and rl_campaign, continued |
//! | ⠯ and | `and_session.rs` | todo, goal, get_context_remaining, code_review, reverse, word_count; code_mode, continued |
//! | ⠮ the | `the_sight.rs` | lsp_definition, lsp_references, lsp_hover, lsp_symbols, lsp_workspace_symbol, ui_inspect, ui_verify; video_cut, continued |
//!
//! Volume III:
//!
//! | cell | chapter                | surface                                              |
//! |------|------------------------|------------------------------------------------------|
//! | ⠻ er | `er_loop.rs`           | the loop iteration: worker, ground, contract, tiers  |
//! | ⠳ ou | `ou_checkpoints.rs`    | loop checkpoints: blocked, overdue, review, done     |
//! | ⠪ ow | `ow_ledgers.rs`        | loop ledgers: changes, measurements, steering, RL    |
//! | ⠜ ar | `ar_seats.rs`          | seats that read the ledger: delegates, spawn seats   |
//! | ⠬ ing| `ing_drivers.rs`       | drivers around a turn: the deli frame, the tutor     |
//! | ⠌ st | `st_connected.rs`      | seats without workspace tools, via the ledger reader |
//!
//! Volume IV, the mixture — a cell without a contraction is named by its dots;
//! every stage is connected (`connect`):
//!
//! | cell  | chapter        | surface                                                      |
//! |-------|----------------|--------------------------------------------------------------|
//! | ⠄ d3  | `d3_roles.rs`  | each stage's brief: proposer, aggregator, classifier, critic, judge, chooser, verifier; hedge, checkpoint, delegator |
//! | ⠈ d4  | `d4_angles.rs` | the six personas, the stances, the wave and refine frames, the research scout |
//! | ⠐ d5  | `d5_frames.rs` | scoring, revise, cite, synthesis, merge, guidance, payload labels, preflight, consults, evidence |
//!
//! Volume V, the long run:
//!
//! | cell  | chapter            | surface                                                   |
//! |-------|--------------------|-----------------------------------------------------------|
//! | ⠘ d45 | `d45_iteration.rs` | an iteration's setbacks, notes, curated state, the brief  |
//! | ⠠ d6  | `d6_long_run.rs`   | the long run's seats: recovery, optimizer, handoff, swarm_compile roles |
//!
//! Volume VI, the replies — a failed or instructive tool result keeps its
//! facts inline (the error, the path, the id, the counts, the next offset,
//! the seconds) and sends each directive as its page address (`⠨⠉⠁`, one
//! sentence) or a long body as its route; an 8-dot cell (dot 7, then dot 8)
//! is a chapter's overflow shelf:
//!
//! | cell   | chapter            | surface                                                      |
//! |--------|--------------------|--------------------------------------------------------------|
//! | ⠨ d46  | `d46_recovery.rs`  | virtual URLs, conflicts, replace misses, hashline, staged edits, shell scope, stopped processes, toolchain, coverage, job output |
//! | ⠰ d56  | `d56_replies.rs`   | list_dir windows, refused seats, seat hints, notes, service limits, work landing, self-map, self-safety, benchmark and jev interpretations |
//! | ⡨ d467 | `d467_receipts.rs` | the handoff brief and action lists; elision marks; the harness's notes on a result |
//!
//! Its skill labels, handle reads and tool notes sit in ⠥ (`⠥⠓ ⠥⠊ ⠥⠚`).
//!
//! The loop ledgers' overflow shelf, ⡪ (dot 7 on ⠪, `d2467_research.rs`),
//! holds the research lifecycle: the Sloptomizer's verdicts (a candidate that
//! passed, beat its baseline, failed or went unverified; a red baseline; a
//! learning error), research or a campaign still live, its advice, a stall with
//! research idle, and a settled campaign. Detectors read them off the run's
//! record and the campaign's status.
//!
//! Volume VII, the seats and the preamble — a seat without workspace tools is
//! connected (`connect`), one with no tool channel hears its pages recited;
//! the preamble's frames are routes, its facts data. A sentence that carries a
//! value is sent as its page address with the value beside it:
//!
//! | cell    | chapter             | surface                                                   |
//! |---------|---------------------|-----------------------------------------------------------|
//! | ⠸ d456  | `d456_knowledge.rs` | the dossier, caddy and project-doc frames; the vision sidecar, the knowledge graph's four seats, the tool-bubble router, the Atlas clerk |
//! | ⡸ d4567 | `d4567_briefs.rs`   | the tutor's local lessons, the pre-provisioner, the Grok research scout, `/mention` and `/skills` |
//!
//! and sections on earlier chapters: ⠬⠋–⠬⠚ (the deli synthesis, the tutor's
//! handoff, recall and fallback, the operator's stand-ins), ⠜⠊ ⠜⠚ (the
//! evaluation rollout's coding agent, Grok's host seat), ⠟⠛–⠟⠚ (the harness's
//! own answers, the teacher-watch), ⠗⠓–⠗⠚ (the goal and memory bounds, the RL
//! grader and prompt optimizer). ⡸ is the 8-dot shelf, dot 7 on ⠸.
//!
//! Volume IX, the seat profiles — a vendor's own words for its model, each
//! section a heading's block of the vendor's prompt, ported whole (sentences as
//! pages, a bullet one page, a block past ten pages continued in a
//! `<name>, continued` section); the seat's knobs are data, not pages. Read
//! through the ledger; the one page sent is ⡞⠙⠓, which an OpenAI seat stands
//! on after its system block:
//!
//! | cell     | chapter              | surface                                                   |
//! |----------|----------------------|-----------------------------------------------------------|
//! | ⡞ d23457 | `d23457_codex_sol.rs` | Codex `gpt-6.1-sol` (CLI 0.159.0): the opening, permission, autonomy, personality, writing style, technical communication, PR descriptions |
//! | ⢞ d23458 | `d23457_codex_sol.rs` | working with the user, commentary, the final answer, formatting, visualizations, the rules for getting work done |
//! | ⣞ d234578 | `d23457_codex_sol.rs` | using skills, when and how to use a skill, apps, plugins, how to use plugins |
//! | ⡯ d123467 | `d23457_codex_sol.rs` | the context window messages: the reminder, the notes guidance, the fallback |
//!
//! Addresses nest: `⠞` a chapter, `⠞⠉` a section (a route), `⠞⠉⠃` a page. A
//! page is one sentence of the prompt it came from, verbatim; a section's pages
//! rebuild that block exactly. Nothing is paraphrased, merged or dropped.
//!
//! A route is two cells, primary then sub (`⠧⠁`: verification, red run). A
//! warpath is up to three routes — six cells — read in order (`⠧⠁⠧⠑⠟⠁`: the
//! last run is red, the green did not hold, if stuck take the deli route).
//! Routes are timed, never always-on: the entry personality type (`⠽`) is
//! the system prompt, the hygiene cue (`⠺`) rides the first tool result of a
//! turn, and after that a warpath appears only when the model's latest
//! actions make a route relevant — on the tail of that tool result, or on its
//! own line at the stop checkpoint and a mode's first hop. Every
//! word lives in the ledger: `read_file ledger://<cells>` decodes a primary,
//! a route or a whole warpath, with the latest evidence. The one standing
//! direction is [`DIRECTION`], a clause in the `read_file` schema, so the
//! cached prefix never moves. The wire introduces each stamp once, in English,
//! where the model first sees it ([`introduction`]): the core type's pages,
//! a route's signal and action, a page's sentence. After that the cells ride
//! alone.
//!
//! Some routes speak in full ([`VOICED`]): the advisories of the 0.1.6 harness,
//! second person and concrete, whose pages the legend gives whole. Each rides a
//! turn of its own: measured at DeepSeek loop points, a cue inside a tool result
//! broke none of 26 loops, the same words as their own turn broke 12 to 18.
//!
//! Detectors throw the routes today; the tree is shaped so a small router
//! model can throw them later.

pub(crate) mod and_session;
pub(crate) mod ar_seats;
pub(crate) mod ch_edits;
pub(crate) mod connect;
pub(crate) mod continuity;
pub(crate) mod d12467_sloptomizer;
pub(crate) mod d23457_codex_sol;
pub(crate) mod d2467_research;
pub(crate) mod d3456_advisories;
pub(crate) mod d3_roles;
pub(crate) mod d4567_briefs;
pub(crate) mod d456_knowledge;
pub(crate) mod d45_iteration;
pub(crate) mod d467_receipts;
pub(crate) mod d46_recovery;
pub(crate) mod d4_angles;
pub(crate) mod d56_replies;
pub(crate) mod d5_frames;
pub(crate) mod d6_long_run;
pub(crate) mod ed_media;
pub(crate) mod er_loop;
pub(crate) mod gh_navigation;
pub(crate) mod ing_drivers;
pub(crate) mod introduction;
pub(crate) mod k_competition;
pub(crate) mod l_loops;
pub(crate) mod ledger;
pub(crate) mod m_method;
pub(crate) mod n_environment;
pub(crate) mod o_orchestration;
pub(crate) mod of_patches;
pub(crate) mod ou_checkpoints;
pub(crate) mod ow_ledgers;
pub(crate) mod p_processes;
pub(crate) mod q_stop;
pub(crate) mod r_relentless;
pub(crate) mod s_sources;
pub(crate) mod sh_shell;
pub(crate) mod st_connected;
pub(crate) mod t_personality;
pub(crate) mod th_team;
pub(crate) mod the_sight;
pub(crate) mod u_skills;
pub(crate) mod v_verification;
pub(crate) mod w_workflow;
pub(crate) mod wh_world;
pub(crate) mod with_build;
pub(crate) mod x_execution;
pub(crate) mod y_types;
pub(crate) mod z_brevity;

use std::path::Path;

/// The one standing direction, carried in the `read_file` schema.
pub(crate) const DIRECTION: &str =
    "braille from the harness is a warpath of routes: `ledger://<cells>` decodes it";

/// Braille numerals: the section and page alphabet (⠁1 … ⠚0).
pub(crate) const DIGITS: [char; 10] = ['⠁', '⠃', '⠉', '⠙', '⠑', '⠋', '⠛', '⠓', '⠊', '⠚'];

/// Most routes in one warpath: six braille characters.
pub(crate) const WARPATH_ROUTES: usize = 3;

/// Tray cell for a hop that raised nothing.
pub(crate) const PASS_GLYPH: char = '⠿';

/// Tray cell for a quiet hop waiting on live background work.
pub(crate) const WAIT_GLYPH: char = '⠤';

/// One route: a primary surface and one of its subcategories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Route {
    pub(crate) primary: char,
    pub(crate) sub: char,
}

impl Route {
    pub(crate) const fn new(primary: char, sub: char) -> Self {
        Self { primary, sub }
    }

    pub(crate) fn cells(self) -> String {
        [self.primary, self.sub].into_iter().collect()
    }

    /// The subcategory this route resolves to on its primary.
    pub(crate) fn sub(self) -> &'static Sub {
        primary(self.primary)
            .and_then(|primary| primary.subs.iter().find(|sub| sub.route == self))
            .expect("every route is attached to a primary surface")
    }

    /// Operator-facing name for the tray and telemetry (`verification.red`).
    pub(crate) fn name(self) -> String {
        let primary = primary(self.primary).map_or("?", |primary| primary.name);
        format!("{primary}.{}", self.sub().name)
    }
}

/// A primary surface: one braille letter, one chapter.
pub(crate) struct Primary {
    pub(crate) cell: char,
    pub(crate) name: &'static str,
    pub(crate) surface: &'static str,
    pub(crate) subs: &'static [Sub],
}

/// A subcategory attached to a primary: what it signals and where it routes.
pub(crate) struct Sub {
    pub(crate) route: Route,
    pub(crate) name: &'static str,
    pub(crate) signal: &'static str,
    pub(crate) action: &'static str,
    pub(crate) ideas: &'static str,
    /// Verbatim pages, one sentence each (a type's pages are addresses).
    pub(crate) pages: &'static [&'static str],
}

/// The first layer, in table-of-contents order.
pub(crate) const TOC: [&Primary; 50] = [
    &k_competition::PRIMARY,
    &l_loops::PRIMARY,
    &m_method::PRIMARY,
    &n_environment::PRIMARY,
    &o_orchestration::PRIMARY,
    &p_processes::PRIMARY,
    &q_stop::PRIMARY,
    &r_relentless::PRIMARY,
    &s_sources::PRIMARY,
    &t_personality::PRIMARY,
    &u_skills::PRIMARY,
    &v_verification::PRIMARY,
    &w_workflow::PRIMARY,
    &x_execution::PRIMARY,
    &y_types::PRIMARY,
    &z_brevity::PRIMARY,
    // Volume II: the tool library.
    &ch_edits::PRIMARY,
    &sh_shell::PRIMARY,
    &th_team::PRIMARY,
    &wh_world::PRIMARY,
    &ed_media::PRIMARY,
    &gh_navigation::PRIMARY,
    &of_patches::PRIMARY,
    &with_build::PRIMARY,
    &and_session::PRIMARY,
    &the_sight::PRIMARY,
    // Volume III: the drivers and their seats.
    &er_loop::PRIMARY,
    &ou_checkpoints::PRIMARY,
    &ow_ledgers::PRIMARY,
    &ar_seats::PRIMARY,
    &ing_drivers::PRIMARY,
    &st_connected::PRIMARY,
    // Volume IV: the mixture.
    &d3_roles::PRIMARY,
    &d4_angles::PRIMARY,
    &d5_frames::PRIMARY,
    // Volume V: the long run.
    &d45_iteration::PRIMARY,
    &d6_long_run::PRIMARY,
    // Volume VI: the replies.
    &d46_recovery::PRIMARY,
    &d56_replies::PRIMARY,
    &d467_receipts::PRIMARY,
    // The loop ledgers' overflow shelf: research.
    &d2467_research::PRIMARY,
    &d12467_sloptomizer::PRIMARY,
    // Volume VII: the seats and the preamble.
    &d456_knowledge::PRIMARY,
    &d4567_briefs::PRIMARY,
    // Volume VIII: the hop advisories, and the competition's shelf.
    &d3456_advisories::PRIMARY,
    &k_competition::SHELF,
    // Volume IX: the seat profiles, Codex gpt-6.1-sol.
    &d23457_codex_sol::PRIMARY,
    &d23457_codex_sol::SHELF_II,
    &d23457_codex_sol::SHELF_III,
    &d23457_codex_sol::CONTEXT,
];

pub(crate) fn primary(cell: char) -> Option<&'static Primary> {
    TOC.into_iter().find(|primary| primary.cell == cell)
}

/// A route a detector threw, with the evidence the ledger should hold for it.
/// A raise may name one page of its route: the stamp then carries that
/// sentence alone, new to a model that already met the route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Raise {
    pub(crate) route: Route,
    pub(crate) page: Option<char>,
    pub(crate) evidence: Option<String>,
    /// The evidence rides beside the stamp as data, not only in the ledger: a
    /// failing run's tail, a receipt, facts the sentences refer to.
    pub(crate) inline: bool,
}

impl Raise {
    pub(crate) fn new(route: Route, evidence: impl Into<Option<String>>) -> Self {
        Self {
            route,
            page: None,
            evidence: evidence.into(),
            inline: false,
        }
    }

    /// A raise whose evidence the model reads beside the stamp.
    pub(crate) fn inline(route: Route, evidence: impl Into<Option<String>>) -> Self {
        Self {
            inline: true,
            ..Self::new(route, evidence)
        }
    }

    /// One page of `route`, by its index among the route's pages.
    pub(crate) fn page(route: Route, page: usize, evidence: impl Into<Option<String>>) -> Self {
        Self {
            route,
            page: DIGITS.get(page).copied(),
            evidence: evidence.into(),
            inline: false,
        }
    }

    /// This raise's stamp: the route's cells, then its page when it names one.
    pub(crate) fn cells(&self) -> String {
        let mut cells = self.route.cells();
        cells.extend(self.page);
        cells
    }
}

/// The routes that speak in full. Their pages are the harness's advice in its
/// own second-person words, carried over from the 0.1.6 advisories, and the
/// legend introduces a route of these with every page rather than its signal
/// and action. The flag marks a warning: a route that breaks a stuck pattern
/// rides its own turn under the warning sign; the rest ride their own turn as
/// bare cells. Either way the advice is a turn of its own, never the tail of a
/// tool result, where a model reads it as more output and moves on.
pub(crate) const VOICED: &[(Route, bool)] = &[
    (l_loops::SAME_BATCH, true),
    (l_loops::STORM, true),
    (l_loops::POLL, true),
    (r_relentless::ARMED, false),
    (p_processes::LIVE, false),
    (p_processes::DONE, false),
    (p_processes::FAILED, false),
    (x_execution::ERRORS, true),
    (x_execution::MARKUP, false),
    (x_execution::OUTPUT_CAP, false),
    (x_execution::REASONING_CAP, false),
    (x_execution::EMPTY, false),
    (v_verification::RED, false),
    (v_verification::ACCEPTANCE, false),
    (v_verification::RED_STREAK, true),
    (v_verification::FLAKY, false),
    (v_verification::UNTESTED, false),
    (v_verification::TESTS_EDITED, false),
    (d3456_advisories::POST_EDIT, false),
    (d3456_advisories::GREEN, false),
    (d3456_advisories::FINAL_MILE, false),
    (d3456_advisories::FIRST_WRITE, false),
    (d3456_advisories::NO_EDIT, false),
    (d3456_advisories::WEAK, false),
    (d3456_advisories::THRASH, true),
    (d3456_advisories::FANOUT, false),
    (d3456_advisories::CASCADE, true),
    (k_competition::RAPID, false),
    (k_competition::DEEP, false),
    (k_competition::WINNER_BANK, false),
    (k_competition::CANDIDATE_CHANGED, false),
    (k_competition::FIRST_WRITE_RAPID, false),
];

/// Whether `route` speaks in full (see [`VOICED`]).
pub(crate) fn is_voiced(route: Route) -> bool {
    VOICED.iter().any(|(voiced, _)| *voiced == route)
}

/// Whether `route` is a voiced warning: its own turn leads with the sign.
pub(crate) fn is_warning(route: Route) -> bool {
    VOICED
        .iter()
        .any(|(voiced, warns)| *voiced == route && *warns)
}

/// The 0.1.6 name of the advisory a route carries, so the experience ledger
/// and the analysis scripts that count `post_edit_logic_advisory`,
/// `verification_recovery`, `error_advisory` and the rest keep counting them.
pub(crate) fn legacy_kind(route: Route) -> Option<&'static str> {
    Some(match route {
        r if r == l_loops::SAME_BATCH => "spin_advisory",
        r if r == l_loops::STORM => "duplicate_storm_advisory",
        r if r == x_execution::ERRORS => "error_advisory",
        r if r == v_verification::RED_STREAK => "verification_recovery",
        r if r == d3456_advisories::POST_EDIT => "post_edit_logic_advisory",
        r if r == d3456_advisories::GREEN => "green_verify_advisory",
        r if r == k_competition::WINNER_BANK => "green_verify_advisory",
        r if r == d3456_advisories::FINAL_MILE => "final_mile_advisory",
        r if r == d3456_advisories::FIRST_WRITE || r == k_competition::FIRST_WRITE_RAPID => {
            "first_write_advisory"
        }
        r if r == d3456_advisories::NO_EDIT => "no_edit_advisory",
        r if r == d3456_advisories::WEAK => "self_authored_verify_advisory",
        r if r == d3456_advisories::THRASH => "mutation_thrash_advisory",
        r if r == d3456_advisories::FANOUT => "peripheral_fanout_advisory",
        _ => return None,
    })
}

/// Encode raises as one warpath: in order, deduplicated, at most
/// [`WARPATH_ROUTES`] routes. Every raise's evidence reaches the ledger, even
/// one past the cap.
pub(crate) fn warpath(workspace: &Path, raises: &[Raise]) -> String {
    let mut stamps: Vec<String> = Vec::new();
    for raise in raises {
        if let Some(evidence) = raise
            .evidence
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            ledger::record(workspace, raise.route, evidence);
        }
        let cells = raise.cells();
        if !stamps.contains(&cells) {
            stamps.push(cells);
        }
    }
    stamps.into_iter().take(WARPATH_ROUTES).collect()
}

/// A turn of its own for these raises: the warpath, then the evidence of each
/// raise that carries it inline, one block per line group. `warning` puts the
/// sign first.
pub(crate) fn advice_turn(workspace: &Path, raises: &[Raise], warning: bool) -> String {
    let mut turn = String::new();
    if warning {
        turn.push(l_loops::WARNING);
    }
    turn.push_str(&warpath(workspace, raises));
    for raise in raises.iter().filter(|raise| raise.inline) {
        if let Some(evidence) = raise
            .evidence
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            turn.push('\n');
            turn.push_str(evidence);
        }
    }
    turn
}

/// Routes as sign lines, [`WARPATH_ROUTES`] to a line, so none is dropped.
pub(crate) fn sign_lines(routes: &[Route]) -> String {
    routes
        .chunks(WARPATH_ROUTES)
        .map(|chunk| chunk.iter().map(|route| route.cells()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Operator-facing names of a warpath's routes, for tray notices.
pub(crate) fn names(raises: &[Raise]) -> String {
    let mut seen = Vec::new();
    raises
        .iter()
        .filter(|raise| {
            let fresh = !seen.contains(&raise.route);
            seen.push(raise.route);
            fresh
        })
        .map(|raise| raise.route.name())
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/book__tests.rs"]
mod tests;
