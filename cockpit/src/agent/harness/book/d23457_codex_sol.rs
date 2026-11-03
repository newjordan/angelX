//! ⡞ ⢞ ⣞ ⡯ (dots 23457, 23458, 234578, 123467) — Volume IX, the seat profiles:
//! what a model's own vendor harness tells it, in its words. This chapter
//! carries OpenAI Codex's profile for `gpt-6.1-sol`.
//!
//! Source: the OpenAI Codex CLI 0.159.0 model catalog, `gpt-6.1-sol`
//! `base_instructions` (Apache-2.0, Codex), and the catalog's context-window
//! message template (`token_budget`); fetched 2026-09-30. The embedded
//! baseline is the vendor's text, ported whole in source order, byte for byte.
//! The tests rebuild it from the `d23457_codex_sol/` copies beside this file.
//! Operator-published catalog overrides may differ and are local, not vendor
//! text.
//!
//! A section is a heading's block; its first page carries the heading, as in
//! `m_method.rs`. A markdown bullet is one page, a nested bullet its own page.
//! A block with more than ten pages continues in a `<name>, continued` section,
//! as Volume II does, and a chapter holds ten sections, so the profile runs
//! over the dot-7 and dot-8 shelves of ⠞ (the Driver's identity and posture: a
//! vendor's own words for the seat are its posture). Every 6-dot cell is
//! taken, so the volume lives on 8-dot cells, the way `⡨ ⡪ ⡸ ⡅` do. The context
//! window messages ride the shelf of ⠯, the chapter of `get_context_remaining`.
//!
//! The seat's knobs (shell type, patch tool, tool mode, verbosity, window
//! sizes) are not voice and are not pages; they are [`SEAT`], a plain constant
//! a driver can read.
//!
//! This is a library, but for one page. An OpenAI seat stands on [`STANDING`],
//! the autonomy page Codex counters sol's own token budgeting with, right after
//! its system block (`openai_codex`, `ANGEL_SEAT_PROFILE`). No other seat is
//! sent these routes, no preamble carries them, no detector throws them; none
//! is voiced (`book::VOICED`), so the legend introduces a stamp of them by its
//! signal, and a page by its sentence, when a seat meets one. The ledger reader
//! decodes them like any chapter. What else rides where is decided later, with
//! a measured A/B.
//!
//! | route | block | pages |
//! |-------|-------|-------|
//! | `⡞⠁` | (the opening, before any heading) | 2 |
//! | `⡞⠃` | # When to ask the user for permission | 10 |
//! | `⡞⠉` | # When to ask the user for permission, continued | 4 |
//! | `⡞⠙` | # Autonomy and persistence | 10 |
//! | `⡞⠑` | # Autonomy and persistence, continued | 3 |
//! | `⡞⠋` | # Personality | 4 |
//! | `⡞⠛` | ## Writing style | 10 |
//! | `⡞⠓` | ## Writing style, continued | 10 |
//! | `⡞⠊` | ## Technical communication | 9 |
//! | `⡞⠚` | ### Writing PR descriptions | 8 |
//! | `⢞⠁` | # Working with the user | 10 |
//! | `⢞⠃` | # Working with the user, continued | 10 |
//! | `⢞⠉` | # Working with the user, continued again | 5 |
//! | `⢞⠙` | ## Intermediate commentary | 9 |
//! | `⢞⠑` | ## Final answer | 1 |
//! | `⢞⠋` | ### Formatting rules | 10 |
//! | `⢞⠛` | ### Formatting rules, continued | 3 |
//! | `⢞⠓` | ### Visualizations | 9 |
//! | `⢞⠊` | # Rules for getting work done | 10 |
//! | `⢞⠚` | # Rules for getting work done, continued | 3 |
//! | `⣞⠁` | # Using skills | 10 |
//! | `⣞⠃` | # Using skills, continued | 2 |
//! | `⣞⠉` | ## When to use a skill | 5 |
//! | `⣞⠙` | ## How to use skills | 5 |
//! | `⣞⠑` | # Apps (Connectors) | 6 |
//! | `⣞⠋` | # Plugins | 1 |
//! | `⣞⠛` | ## How to use plugins | 6 |
//! | `⡯⠁` | token_budget.reminder_message_template | 8 |
//! | `⡯⠃` | token_budget.guidance_message | 10 |
//! | `⡯⠉` | token_budget.guidance_message, continued | 4 |
//! | `⡯⠙` | token_budget.auto_compact_fallback_prompt | 7 |

use super::{Primary, Route, Sub};
const PROFILE_PRIMARY: &str = include_str!("../../../../book/codex_sol_primary.book");
const PROFILE_SHELF_II: &str = include_str!("../../../../book/codex_sol_shelf_ii.book");
const PROFILE_SHELF_III: &str = include_str!("../../../../book/codex_sol_shelf_iii.book");
const PROFILE_CONTEXT: &str = include_str!("../../../../book/codex_sol_context.book");

/// ⡞ (d23457): the first shelf of ⠞, where the profile opens.
pub(crate) const CELL: char = '⡞';
/// ⢞ (d23458): the second shelf of ⠞: the profile continues.
pub(crate) const SHELF_II_CELL: char = '⢞';
/// ⣞ (d234578): the third shelf of ⠞: the profile continues.
pub(crate) const SHELF_III_CELL: char = '⣞';
/// ⡯ (d123467): the shelf of ⠯, the context window messages.
pub(crate) const CONTEXT_CELL: char = '⡯';

pub(crate) const OPENING: Route = Route::new(CELL, '⠁');
pub(crate) const PERMISSION: Route = Route::new(CELL, '⠃');
pub(crate) const PERMISSION_MORE: Route = Route::new(CELL, '⠉');
pub(crate) const AUTONOMY: Route = Route::new(CELL, '⠙');
pub(crate) const AUTONOMY_MORE: Route = Route::new(CELL, '⠑');
pub(crate) const PERSONALITY: Route = Route::new(CELL, '⠋');
pub(crate) const WRITING_STYLE: Route = Route::new(CELL, '⠛');
pub(crate) const WRITING_STYLE_MORE: Route = Route::new(CELL, '⠓');
pub(crate) const TECHNICAL: Route = Route::new(CELL, '⠊');
pub(crate) const PR_DESCRIPTIONS: Route = Route::new(CELL, '⠚');
pub(crate) const WORKING: Route = Route::new(SHELF_II_CELL, '⠁');
pub(crate) const WORKING_MORE: Route = Route::new(SHELF_II_CELL, '⠃');
pub(crate) const WORKING_MORE2: Route = Route::new(SHELF_II_CELL, '⠉');
pub(crate) const COMMENTARY: Route = Route::new(SHELF_II_CELL, '⠙');
pub(crate) const FINAL_ANSWER: Route = Route::new(SHELF_II_CELL, '⠑');
pub(crate) const FORMATTING: Route = Route::new(SHELF_II_CELL, '⠋');
pub(crate) const FORMATTING_MORE: Route = Route::new(SHELF_II_CELL, '⠛');
pub(crate) const VISUALIZATIONS: Route = Route::new(SHELF_II_CELL, '⠓');
pub(crate) const GETTING_WORK_DONE: Route = Route::new(SHELF_II_CELL, '⠊');
pub(crate) const GETTING_WORK_DONE_MORE: Route = Route::new(SHELF_II_CELL, '⠚');
pub(crate) const USING_SKILLS: Route = Route::new(SHELF_III_CELL, '⠁');
pub(crate) const USING_SKILLS_MORE: Route = Route::new(SHELF_III_CELL, '⠃');
pub(crate) const WHEN_SKILL: Route = Route::new(SHELF_III_CELL, '⠉');
pub(crate) const HOW_SKILLS: Route = Route::new(SHELF_III_CELL, '⠙');
pub(crate) const APPS: Route = Route::new(SHELF_III_CELL, '⠑');
pub(crate) const PLUGINS: Route = Route::new(SHELF_III_CELL, '⠋');
pub(crate) const HOW_PLUGINS: Route = Route::new(SHELF_III_CELL, '⠛');
pub(crate) const CONTEXT_REMINDER: Route = Route::new(CONTEXT_CELL, '⠁');
pub(crate) const CONTEXT_GUIDANCE: Route = Route::new(CONTEXT_CELL, '⠃');
pub(crate) const CONTEXT_GUIDANCE_MORE: Route = Route::new(CONTEXT_CELL, '⠉');
pub(crate) const CONTEXT_FALLBACK: Route = Route::new(CONTEXT_CELL, '⠙');

/// The page an OpenAI seat stands on, by its index in ⡞⠙ (`⡞⠙⠓`): "Do not
/// settle for a partial or "helpful enough" solution that does not fully
/// satisfy the user's task to save time, effort or tokens." Sol's reasoning
/// keeps a token budget of its own and wraps up to it; this is the sentence
/// Codex answers that habit with.
pub(crate) const STANDING: (Route, usize) = (AUTONOMY, 7);

/// What the Codex catalog fixes about a seat, as plain data. These are knobs,
/// not voice: no page carries them, and a driver reads them to shape a seat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct SeatProfile {
    /// The catalog slug the model answers to.
    pub(crate) slug: &'static str,
    /// The name the catalog shows for it.
    pub(crate) display_name: &'static str,
    /// The reasoning effort a turn starts on.
    pub(crate) default_reasoning_level: &'static str,
    /// Every reasoning effort the seat takes, lightest first.
    pub(crate) reasoning_levels: &'static [&'static str],
    /// Whether the effort can change between turns of one session.
    pub(crate) reasoning_effort_updates: bool,
    /// The reasoning summary a turn asks for (`none`: no summary).
    pub(crate) default_reasoning_summary: &'static str,
    /// The answer verbosity a turn starts on.
    pub(crate) default_verbosity: &'static str,
    /// Whether the seat takes a verbosity setting at all.
    pub(crate) supports_verbosity: bool,
    /// How the seat runs shell commands (`unified_exec`: one exec tool).
    pub(crate) shell_type: &'static str,
    /// How the seat edits files (`freeform`: a freeform patch tool, not JSON arguments).
    pub(crate) apply_patch_tool_type: &'static str,
    /// The web search tool's result shape.
    pub(crate) web_search_tool_type: &'static str,
    /// The tool surface (`code_mode_only`: the seat works through code mode alone).
    pub(crate) tool_mode: &'static str,
    /// Whether the seat can search for tools it was not handed.
    pub(crate) supports_search_tool: bool,
    /// Extra tools the catalog marks experimental for this seat.
    pub(crate) experimental_tools: &'static [&'static str],
    /// Whether the seat is spoken to over the lite Responses transport.
    pub(crate) use_responses_lite: bool,
    /// What a tool result is cut to before the seat reads it: the unit.
    pub(crate) truncation_mode: &'static str,
    /// What a tool result is cut to before the seat reads it: the size, in that unit.
    pub(crate) truncation_limit: u32,
    /// The context window, in tokens.
    pub(crate) context_window: u32,
    /// The largest context window the seat can be raised to, in tokens.
    pub(crate) max_context_window: u32,
    /// The share of the window a turn may fill before compaction, in percent.
    pub(crate) effective_context_window_percent: u8,
    /// What the seat reads.
    pub(crate) input_modalities: &'static [&'static str],
    /// Whether the seat takes an image at its original detail.
    pub(crate) supports_image_detail_original: bool,
    /// The multi-agent protocol version the seat speaks.
    pub(crate) multi_agent_version: &'static str,
    /// The reasoning effort its delegates run on.
    pub(crate) multi_agent_reasoning_effort: &'static str,
    /// Whether the catalog puts the skills usage text in the seat's instructions.
    pub(crate) include_skills_usage_instructions: bool,
    /// Whether the catalog puts the plugin usage text in the seat's instructions.
    pub(crate) include_plugin_usage_instructions: bool,
    /// Whether the catalog puts the apps usage text in the seat's instructions.
    pub(crate) include_apps_usage_instructions: bool,
    /// Whether the context-window reminder is switched on for the seat.
    pub(crate) context_reminder_enabled: bool,
    /// Tokens left in the window at which the reminder is sent.
    pub(crate) context_reminder_threshold_tokens: u32,
    /// Whether the history tool extends the seat's notes.
    pub(crate) history_notes_extension: bool,
    /// Tokens kept back for the notes call when the window is spent.
    pub(crate) context_fallback_buffer_tokens: u32,
}

/// The Codex catalog's profile for `gpt-6.1-sol` (Codex CLI 0.159.0, fetched
/// 2026-09-30): the catalog entry and the context-window message's settings.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const SEAT: SeatProfile = SeatProfile {
    slug: "gpt-6.1-sol",
    display_name: "GPT-6.1-Sol",
    default_reasoning_level: "low",
    reasoning_levels: &["low", "medium", "high", "xhigh", "max", "ultra"],
    reasoning_effort_updates: true,
    default_reasoning_summary: "none",
    default_verbosity: "low",
    supports_verbosity: true,
    shell_type: "unified_exec",
    apply_patch_tool_type: "freeform",
    web_search_tool_type: "text_and_image",
    tool_mode: "code_mode_only",
    supports_search_tool: true,
    experimental_tools: &["send_user_message_async", "clock"],
    use_responses_lite: true,
    truncation_mode: "tokens",
    truncation_limit: 10_000,
    context_window: 272_000,
    max_context_window: 872_000,
    effective_context_window_percent: 95,
    input_modalities: &["text", "image"],
    supports_image_detail_original: true,
    multi_agent_version: "v2",
    multi_agent_reasoning_effort: "xhigh",
    include_skills_usage_instructions: false,
    include_plugin_usage_instructions: false,
    include_apps_usage_instructions: false,
    context_reminder_enabled: false,
    context_reminder_threshold_tokens: 6_144,
    history_notes_extension: false,
    context_fallback_buffer_tokens: 16_384,
};

/// The vendor's text, as committed beside this chapter; the tests rebuild every
/// section from it.
#[cfg(test)]
pub(crate) const SOURCE_BASE: &str = include_str!("d23457_codex_sol/sol-base-instructions.md");
#[cfg(test)]
pub(crate) const SOURCE_TOKEN_BUDGET: &str =
    include_str!("d23457_codex_sol/sol-model-message-token_budget.md");
#[cfg(test)]
pub(crate) const SOURCE_KNOBS: &str = include_str!("d23457_codex_sol/sol-catalog-knobs.json");

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "codex-sol",
    surface: "the seat profiles: what Codex tells gpt-6.1-sol, in its words — its opening, permission, autonomy, personality, writing style, technical communication and PR descriptions",
    subs: &[
        Sub {
            route: OPENING,
            name: "opening",
            signal: "what Codex tells gpt-6.1-sol on who it is and the workspace it shares with the user",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(PROFILE_PRIMARY, b'1'),
        },
        Sub {
            route: PERMISSION,
            name: "permission",
            signal: "what Codex tells gpt-6.1-sol on when to ask the user for permission",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(PROFILE_PRIMARY, b'2'),
        },
        Sub {
            route: PERMISSION_MORE,
            name: "permission, continued",
            signal: "what Codex tells gpt-6.1-sol on when to ask the user for permission, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(PROFILE_PRIMARY, b'3'),
        },
        Sub {
            route: AUTONOMY,
            name: "autonomy",
            signal: "what Codex tells gpt-6.1-sol on autonomy and persistence",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(PROFILE_PRIMARY, b'4'),
        },
        Sub {
            route: AUTONOMY_MORE,
            name: "autonomy, continued",
            signal: "what Codex tells gpt-6.1-sol on autonomy and persistence, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(PROFILE_PRIMARY, b'5'),
        },
        Sub {
            route: PERSONALITY,
            name: "personality",
            signal: "what Codex tells gpt-6.1-sol on its personality",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(PROFILE_PRIMARY, b'6'),
        },
        Sub {
            route: WRITING_STYLE,
            name: "writing-style",
            signal: "what Codex tells gpt-6.1-sol on its writing style",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(PROFILE_PRIMARY, b'7'),
        },
        Sub {
            route: WRITING_STYLE_MORE,
            name: "writing-style, continued",
            signal: "what Codex tells gpt-6.1-sol on its writing style, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(PROFILE_PRIMARY, b'8'),
        },
        Sub {
            route: TECHNICAL,
            name: "technical-communication",
            signal: "what Codex tells gpt-6.1-sol on technical communication",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(PROFILE_PRIMARY, b'9'),
        },
        Sub {
            route: PR_DESCRIPTIONS,
            name: "pr-descriptions",
            signal: "what Codex tells gpt-6.1-sol on writing PR descriptions",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(PROFILE_PRIMARY, b'0'),
        },
    ],
};

pub(crate) const SHELF_II: Primary = Primary {
    cell: SHELF_II_CELL,
    name: "codex-sol-2",
    surface: "the seat profiles, continued: working with the user, commentary, the final answer, formatting, visualizations and the rules for getting work done",
    subs: &[
        Sub {
            route: WORKING,
            name: "working-with-the-user",
            signal: "what Codex tells gpt-6.1-sol on working with the user",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(PROFILE_SHELF_II, b'1'),
        },
        Sub {
            route: WORKING_MORE,
            name: "working-with-the-user, continued",
            signal: "what Codex tells gpt-6.1-sol on working with the user, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(PROFILE_SHELF_II, b'2'),
        },
        Sub {
            route: WORKING_MORE2,
            name: "working-with-the-user, continued again",
            signal: "what Codex tells gpt-6.1-sol on working with the user, continued again",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(PROFILE_SHELF_II, b'3'),
        },
        Sub {
            route: COMMENTARY,
            name: "commentary",
            signal: "what Codex tells gpt-6.1-sol on intermediate commentary",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(PROFILE_SHELF_II, b'4'),
        },
        Sub {
            route: FINAL_ANSWER,
            name: "final-answer",
            signal: "what Codex tells gpt-6.1-sol on the final answer",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(PROFILE_SHELF_II, b'5'),
        },
        Sub {
            route: FORMATTING,
            name: "formatting-rules",
            signal: "what Codex tells gpt-6.1-sol on formatting the final answer",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(PROFILE_SHELF_II, b'6'),
        },
        Sub {
            route: FORMATTING_MORE,
            name: "formatting-rules, continued",
            signal: "what Codex tells gpt-6.1-sol on formatting the final answer, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(PROFILE_SHELF_II, b'7'),
        },
        Sub {
            route: VISUALIZATIONS,
            name: "visualizations",
            signal: "what Codex tells gpt-6.1-sol on visualizations",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<9>(PROFILE_SHELF_II, b'8'),
        },
        Sub {
            route: GETTING_WORK_DONE,
            name: "getting-work-done",
            signal: "what Codex tells gpt-6.1-sol on the rules for getting work done",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(PROFILE_SHELF_II, b'9'),
        },
        Sub {
            route: GETTING_WORK_DONE_MORE,
            name: "getting-work-done, continued",
            signal: "what Codex tells gpt-6.1-sol on the rules for getting work done, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<3>(PROFILE_SHELF_II, b'0'),
        },
    ],
};

pub(crate) const SHELF_III: Primary = Primary {
    cell: SHELF_III_CELL,
    name: "codex-sol-3",
    surface: "the seat profiles, continued: skills, apps and plugins",
    subs: &[
        Sub {
            route: USING_SKILLS,
            name: "using-skills",
            signal: "what Codex tells gpt-6.1-sol on using skills",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(PROFILE_SHELF_III, b'1'),
        },
        Sub {
            route: USING_SKILLS_MORE,
            name: "using-skills, continued",
            signal: "what Codex tells gpt-6.1-sol on using skills, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<2>(PROFILE_SHELF_III, b'2'),
        },
        Sub {
            route: WHEN_SKILL,
            name: "when-to-use-a-skill",
            signal: "what Codex tells gpt-6.1-sol on when to use a skill",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(PROFILE_SHELF_III, b'3'),
        },
        Sub {
            route: HOW_SKILLS,
            name: "how-to-use-skills",
            signal: "what Codex tells gpt-6.1-sol on how to use skills",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<5>(PROFILE_SHELF_III, b'4'),
        },
        Sub {
            route: APPS,
            name: "apps",
            signal: "what Codex tells gpt-6.1-sol on apps (connectors)",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(PROFILE_SHELF_III, b'5'),
        },
        Sub {
            route: PLUGINS,
            name: "plugins",
            signal: "what Codex tells gpt-6.1-sol on plugins",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<1>(PROFILE_SHELF_III, b'6'),
        },
        Sub {
            route: HOW_PLUGINS,
            name: "how-to-use-plugins",
            signal: "what Codex tells gpt-6.1-sol on how to use plugins",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<6>(PROFILE_SHELF_III, b'7'),
        },
    ],
};

pub(crate) const CONTEXT: Primary = Primary {
    cell: CONTEXT_CELL,
    name: "codex-sol-context",
    surface: "the seat profiles: the context window messages Codex sends gpt-6.1-sol, the reminder, the notes guidance and the fallback",
    subs: &[
        Sub {
            route: CONTEXT_REMINDER,
            name: "context-reminder",
            signal: "what Codex tells gpt-6.1-sol on the context window reminder, sent when the window is nearly spent",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<8>(PROFILE_CONTEXT, b'1'),
        },
        Sub {
            route: CONTEXT_GUIDANCE,
            name: "context-guidance",
            signal: "what Codex tells gpt-6.1-sol on keeping notes across context windows",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<10>(PROFILE_CONTEXT, b'2'),
        },
        Sub {
            route: CONTEXT_GUIDANCE_MORE,
            name: "context-guidance, continued",
            signal: "what Codex tells gpt-6.1-sol on keeping notes across context windows, continued",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<4>(PROFILE_CONTEXT, b'3'),
        },
        Sub {
            route: CONTEXT_FALLBACK,
            name: "context-fallback",
            signal: "what Codex tells gpt-6.1-sol on the context window fallback, sent when the window is spent",
            action: "",
            ideas: "",
            pages: &super::catalog_pages::<7>(PROFILE_CONTEXT, b'4'),
        },
    ],
};

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/book__seat_profiles_tests.rs"]
mod tests;
