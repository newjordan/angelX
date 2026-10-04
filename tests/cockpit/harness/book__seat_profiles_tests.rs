//! Volume IX, the seat profiles: Codex's `gpt-6.1-sol` profile in its own words.
//! Every heading's block of the vendor's text is rebuilt, byte for byte, from
//! its section's pages; the seat's knobs are read back off the catalog; and the
//! chapter stays a library but for the one page an OpenAI seat stands on.
use super::*;
use crate::agent::club::ChatMsg;
use crate::agent::harness::book::{self, DIGITS, TOC, connect, introduction, ledger};

/// The source's blocks, in order: the text before the first heading, then each
/// `#`/`##`/`###` heading with everything up to the next.
fn blocks(source: &str) -> Vec<&str> {
    let mut starts = vec![0];
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        if offset > 0 && ["# ", "## ", "### "].iter().any(|h| line.starts_with(h)) {
            starts.push(offset);
        }
        offset += line.len();
    }
    starts.push(source.len());
    starts.windows(2).map(|w| &source[w[0]..w[1]]).collect()
}

/// Whether `pages`, in order, are the block exactly: each page is the next text
/// of the block, byte for byte, and only whitespace lies between them.
fn rebuilds(block: &str, pages: &[&str]) -> Result<(), String> {
    let mut rest = block;
    for (index, page) in pages.iter().enumerate() {
        rest = rest.trim_start();
        rest = rest
            .strip_prefix(page)
            .ok_or_else(|| format!("page {index} is not the block's next text: {page:?}"))?;
    }
    if rest.trim().is_empty() {
        Ok(())
    } else {
        Err(format!("the block's tail is on no page: {rest:?}"))
    }
}

fn pages_of(routes: &[Route]) -> Vec<&'static str> {
    routes
        .iter()
        .flat_map(|route| route.sub().pages.iter().copied())
        .collect()
}

/// Each block of the base instructions, with the sections that carry it.
const BASE_BLOCKS: &[&[Route]] = &[
    &[OPENING],
    &[PERMISSION, PERMISSION_MORE],
    &[AUTONOMY, AUTONOMY_MORE],
    &[PERSONALITY],
    &[WRITING_STYLE, WRITING_STYLE_MORE],
    &[TECHNICAL],
    &[PR_DESCRIPTIONS],
    &[WORKING, WORKING_MORE, WORKING_MORE2],
    &[COMMENTARY],
    &[FINAL_ANSWER],
    &[FORMATTING, FORMATTING_MORE],
    &[VISUALIZATIONS],
    &[GETTING_WORK_DONE, GETTING_WORK_DONE_MORE],
    &[USING_SKILLS, USING_SKILLS_MORE],
    &[WHEN_SKILL],
    &[HOW_SKILLS],
    &[APPS],
    &[PLUGINS],
    &[HOW_PLUGINS],
];

/// Each message of the context-window template, with the sections that carry it.
const TOKEN_BUDGET_BLOCKS: &[(&str, &[Route])] = &[
    ("reminder_message_template", &[CONTEXT_REMINDER]),
    (
        "guidance_message",
        &[CONTEXT_GUIDANCE, CONTEXT_GUIDANCE_MORE],
    ),
    ("auto_compact_fallback_prompt", &[CONTEXT_FALLBACK]),
];

fn all_routes() -> Vec<Route> {
    BASE_BLOCKS
        .iter()
        .flat_map(|routes| routes.iter().copied())
        .chain(
            TOKEN_BUDGET_BLOCKS
                .iter()
                .flat_map(|(_, routes)| routes.iter().copied()),
        )
        .collect()
}

#[test]
fn every_heading_block_of_the_base_instructions_is_rebuilt_from_its_pages() {
    let source = blocks(SOURCE_BASE);
    assert_eq!(
        source.len(),
        BASE_BLOCKS.len(),
        "a block of the source is on no section, or a section has no block"
    );
    // The headings, in order: what a section follows.
    let headings = source
        .iter()
        .filter_map(|block| block.lines().next().filter(|line| line.starts_with('#')))
        .count();
    assert_eq!(headings, 18);
    for (block, routes) in source.iter().zip(BASE_BLOCKS) {
        let pages = pages_of(routes);
        rebuilds(block, &pages).unwrap_or_else(|error| panic!("{}: {error}", routes[0].name()));
        // A block that opens on a heading carries it on its first page.
        if block.starts_with('#') {
            assert!(pages[0].starts_with(block.lines().next().unwrap()));
        }
    }
    // The whole prompt: nothing between the blocks is lost.
    let every_page = BASE_BLOCKS
        .iter()
        .flat_map(|routes| pages_of(routes))
        .collect::<Vec<_>>();
    rebuilds(SOURCE_BASE, &every_page).expect("the base instructions, whole");
}

#[test]
fn every_message_of_the_context_window_template_is_rebuilt_from_its_pages() {
    let template: serde_json::Value = serde_json::from_str(SOURCE_TOKEN_BUDGET).unwrap();
    for (key, routes) in TOKEN_BUDGET_BLOCKS {
        let text = template[*key].as_str().unwrap_or_else(|| panic!("{key}"));
        rebuilds(text, &pages_of(routes)).unwrap_or_else(|error| panic!("{key}: {error}"));
    }
    // The reminder's placeholder is the one slot, its value beside the page.
    assert!(CONTEXT_REMINDER.sub().pages[0].contains("{n_remaining}"));
}

#[test]
fn pages_are_the_texts_own_and_never_padded() {
    for route in all_routes() {
        let sub = route.sub();
        assert!(!sub.pages.is_empty(), "{}", route.name());
        assert!(sub.pages.len() <= DIGITS.len(), "{}", route.name());
        assert!(
            sub.action.is_empty() && sub.ideas.is_empty(),
            "{}",
            route.name()
        );
        for page in sub.pages {
            assert_eq!(page.trim(), *page, "{}: a padded page", route.name());
            assert!(!page.is_empty(), "{}", route.name());
        }
    }
}

#[test]
fn the_volume_is_four_eight_dot_cells_in_order_at_the_end_of_the_book() {
    let cells = [CELL, SHELF_II_CELL, SHELF_III_CELL, CONTEXT_CELL];
    assert_eq!(
        TOC.iter()
            .rev()
            .take(4)
            .rev()
            .map(|p| p.cell)
            .collect::<Vec<_>>(),
        cells
    );
    for cell in cells {
        // Dot 7 or dot 8 lifts a cell out of the 6-dot alphabet, which is full.
        assert!(ledger::is_cell(cell) && (cell as u32 - 0x2800) >= 0x40);
        assert_eq!(
            TOC.iter().filter(|primary| primary.cell == cell).count(),
            1,
            "{cell} is one chapter"
        );
    }
    // Dot 7 and dot 8 on ⠞, and dot 7 on ⠯.
    assert_eq!(
        cells.map(|cell| cell as u32 - 0x2800),
        [0x5E, 0x9E, 0xDE, 0x6F]
    );
    let sizes = [PRIMARY, SHELF_II, SHELF_III, CONTEXT].map(|primary| primary.subs.len());
    assert_eq!(sizes, [10, 10, 7, 4]);
    for primary in [PRIMARY, SHELF_II, SHELF_III, CONTEXT] {
        for (index, sub) in primary.subs.iter().enumerate() {
            assert_eq!(sub.route.primary, primary.cell, "{}", sub.name);
            assert_eq!(sub.route.sub, DIGITS[index], "{} out of order", sub.name);
            assert_eq!(sub.route.sub().name, sub.name);
        }
    }
    // Every section is on a block: 27 for the base, 4 for the context messages.
    assert_eq!(all_routes().len(), 31);
    assert_eq!(all_routes().len(), sizes.iter().sum::<usize>());
}

#[test]
fn a_block_past_ten_pages_continues_in_a_section_named_for_it() {
    for routes in BASE_BLOCKS
        .iter()
        .copied()
        .chain(TOKEN_BUDGET_BLOCKS.iter().map(|(_, routes)| *routes))
    {
        let name = routes[0].sub().name;
        assert!(!name.contains("continued"), "{name}");
        for (index, route) in routes.iter().enumerate() {
            let sub = route.sub();
            match index {
                0 => {}
                1 => assert_eq!(sub.name, format!("{name}, continued")),
                _ => assert_eq!(sub.name, format!("{name}, continued again")),
            }
            // A section is continued only once it is full.
            if index + 1 < routes.len() {
                assert_eq!(sub.pages.len(), DIGITS.len(), "{} is full", sub.name);
            } else {
                assert!(sub.pages.len() <= DIGITS.len());
            }
        }
    }
}

#[test]
fn the_ledger_and_the_reader_give_every_page() {
    let root = std::env::temp_dir();
    let toc = ledger::read(&root, "").unwrap();
    for primary in [PRIMARY, SHELF_II, SHELF_III, CONTEXT] {
        assert!(toc.contains(primary.cell) && toc.contains(primary.name));
        let chapter = ledger::read(&root, &primary.cell.to_string()).unwrap();
        for sub in primary.subs {
            assert!(chapter.contains(&sub.route.cells()), "{}", sub.name);
        }
    }
    for route in all_routes() {
        let sub = route.sub();
        let section = ledger::read(&root, &route.cells()).unwrap();
        for (index, page) in sub.pages.iter().enumerate() {
            assert!(section.contains(page), "{} page {index}", route.name());
            let one = ledger::read(&root, &format!("{}{}", route.cells(), DIGITS[index])).unwrap();
            assert!(one.ends_with(page), "{} page {index}", route.name());
        }
        // A seat with no tool channel hears the pages themselves, verbatim.
        assert_eq!(connect::recite(&route.cells()), sub.pages.join(" "));
    }
    // A warpath of these reads like any other.
    let path = ledger::read(&root, "⡞⠃⢞⠁⡯⠁").unwrap();
    assert!(path.contains("When to ask the user for permission"));
    assert!(path.contains("Working with the user"));
    assert!(path.contains("{n_remaining}"));
}

fn legend_of(messages: &[ChatMsg]) -> Vec<String> {
    introduction::introductions(messages, None)
        .into_iter()
        .map(|(_, text)| text)
        .collect()
}

#[test]
fn a_stamp_of_the_profile_is_introduced_by_its_signal_and_a_page_by_its_sentence() {
    for route in all_routes() {
        let sub = route.sub();
        assert!(!book::is_voiced(route), "{} is a library", route.name());
        assert!(!book::is_warning(route));
        assert_eq!(book::legacy_kind(route), None);
        // A section: its signal, once.
        let messages = [
            ChatMsg::user("go".to_string()),
            ChatMsg::harness(route.cells()),
            ChatMsg::harness(route.cells()),
        ];
        let intros = legend_of(&messages);
        assert_eq!(intros.len(), 1, "{}: once", route.name());
        assert!(
            intros[0].contains(&format!("{} {}", route.cells(), sub.signal)),
            "{}: {}",
            route.name(),
            intros[0]
        );
        // A page: its sentence, verbatim, as it is written.
        for (index, page) in sub.pages.iter().enumerate() {
            let stamp = format!("{}{}", route.cells(), DIGITS[index]);
            let intros = legend_of(&[ChatMsg::harness(stamp.clone())]);
            assert_eq!(intros.len(), 1, "{stamp}");
            let intro = &intros[0];
            if page.contains("{n_remaining}") {
                let met = page.replace("{n_remaining}", "…");
                assert!(
                    intro.ends_with(&format!("{stamp} {met}")),
                    "{stamp}: {intro}"
                );
            } else {
                assert!(
                    intro.ends_with(&format!("{stamp} {page}")),
                    "{stamp}: {intro}"
                );
            }
        }
    }
    // The vendor's literal braces are not read as a value slot.
    let apps = legend_of(&[ChatMsg::harness("⣞⠑⠁".to_string())]);
    assert!(apps[0].contains("app://{{connector_id}})"), "{apps:?}");
}

#[test]
fn the_seat_profile_is_the_catalog_entry() {
    let knobs: serde_json::Value = serde_json::from_str(SOURCE_KNOBS).unwrap();
    let budget: serde_json::Value = serde_json::from_str(SOURCE_TOKEN_BUDGET).unwrap();
    let text = |key: &str| knobs[key].as_str().unwrap_or_else(|| panic!("{key}"));
    let flag = |key: &str| knobs[key].as_bool().unwrap_or_else(|| panic!("{key}"));
    let number = |value: &serde_json::Value, key: &str| {
        value[key].as_u64().unwrap_or_else(|| panic!("{key}"))
    };
    let list = |key: &str| {
        knobs[key]
            .as_array()
            .unwrap_or_else(|| panic!("{key}"))
            .iter()
            .map(|item| item.as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    let strs = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(SEAT.slug, text("slug"));
    assert_eq!(SEAT.display_name, text("display_name"));
    assert_eq!(
        SEAT.default_reasoning_level,
        text("default_reasoning_level")
    );
    assert_eq!(
        strs(SEAT.reasoning_levels),
        knobs["supported_reasoning_levels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|level| level["effort"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        SEAT.reasoning_effort_updates,
        flag("supports_reasoning_effort_updates")
    );
    assert_eq!(
        SEAT.default_reasoning_summary,
        text("default_reasoning_summary")
    );
    assert_eq!(SEAT.default_verbosity, text("default_verbosity"));
    assert_eq!(SEAT.supports_verbosity, flag("support_verbosity"));
    assert_eq!(SEAT.shell_type, text("shell_type"));
    assert_eq!(SEAT.apply_patch_tool_type, text("apply_patch_tool_type"));
    assert_eq!(SEAT.web_search_tool_type, text("web_search_tool_type"));
    assert_eq!(SEAT.tool_mode, text("tool_mode"));
    assert_eq!(SEAT.supports_search_tool, flag("supports_search_tool"));
    assert_eq!(
        strs(SEAT.experimental_tools),
        list("experimental_supported_tools")
    );
    assert_eq!(SEAT.use_responses_lite, flag("use_responses_lite"));
    assert_eq!(
        SEAT.truncation_mode,
        knobs["truncation_policy"]["mode"].as_str().unwrap()
    );
    assert_eq!(
        u64::from(SEAT.truncation_limit),
        number(&knobs["truncation_policy"], "limit")
    );
    assert_eq!(
        u64::from(SEAT.context_window),
        number(&knobs, "context_window")
    );
    assert_eq!(
        u64::from(SEAT.max_context_window),
        number(&knobs, "max_context_window")
    );
    assert_eq!(
        u64::from(SEAT.effective_context_window_percent),
        number(&knobs, "effective_context_window_percent")
    );
    assert_eq!(strs(SEAT.input_modalities), list("input_modalities"));
    assert_eq!(
        SEAT.supports_image_detail_original,
        flag("supports_image_detail_original")
    );
    assert_eq!(SEAT.multi_agent_version, text("multi_agent_version"));
    assert_eq!(
        SEAT.multi_agent_reasoning_effort,
        text("multi_agent_reasoning_effort")
    );
    assert_eq!(
        SEAT.include_skills_usage_instructions,
        flag("include_skills_usage_instructions")
    );
    assert_eq!(
        SEAT.include_plugin_usage_instructions,
        flag("include_plugin_usage_instructions")
    );
    assert_eq!(
        SEAT.include_apps_usage_instructions,
        flag("include_apps_usage_instructions")
    );
    // The context-window message's settings.
    assert_eq!(
        SEAT.context_reminder_enabled,
        budget["enabled"].as_bool().unwrap()
    );
    assert_eq!(
        u64::from(SEAT.context_reminder_threshold_tokens),
        number(&budget, "reminder_threshold_tokens")
    );
    assert_eq!(
        SEAT.history_notes_extension,
        budget["use_history_notes_extension"].as_bool().unwrap()
    );
    assert_eq!(
        u64::from(SEAT.context_fallback_buffer_tokens),
        number(&budget, "auto_compact_fallback_buffer_tokens")
    );
}

#[test]
fn only_the_standing_page_rides_a_seat() {
    // A library but for one page: no detector throws these routes and no
    // preamble names them. An OpenAI seat stands on the autonomy page Codex
    // answers sol's own token budgeting with.
    for route in all_routes() {
        assert!(!book::VOICED.iter().any(|(voiced, _)| *voiced == route));
    }
    let (route, page) = STANDING;
    assert_eq!(route, AUTONOMY);
    assert_eq!(
        route.sub().pages[page],
        "Do not settle for a partial or \"helpful enough\" solution that does not fully satisfy the user's task to save time, effort or tokens."
    );
    assert_eq!(book::Raise::page(route, page, None).cells(), "⡞⠙⠓");
}
