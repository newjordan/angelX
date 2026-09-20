//! Z4 quest-HUD tests: the composer, its width ladder, the pane border, and
//! the region prefix on the title and the world status strip.

use super::super::adventure::{AdventureEvent, LoopKind};
use super::super::{LoopBudgetSnapshot, World, cell_width};
use crate::hud::{HUD_AMBER, HUD_DIM, HUD_GOLD};

/// Flatten a composed line back to the text a terminal would show.
fn flat(line: &ratatui::text::Line<'static>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

/// Comp / lean mode is process-global env: serialize and clear it, then let
/// the guard put the operator's posture back.
fn unlean() -> (
    std::sync::MutexGuard<'static, ()>,
    crate::tests::TestEnvGuard,
    crate::tests::TestEnvGuard,
) {
    let lock = crate::tests::env_lock();
    let comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
    let turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
    crate::comp_mode::invalidate_cache();
    (lock, comp, turbo)
}

/// The spec's worked example: the Mines at iteration 7, two treasures and one
/// empty chest, danger 1, a party of three, the treasure banner still up.
fn spec_example_world() -> World {
    let mut world = World::new(3);
    world.note_adventure(AdventureEvent::LoopStarted {
        kind: LoopKind::Competition,
        task: "kernel benchmark".to_string(),
    });
    world.note_adventure(AdventureEvent::Party { size: 3 });
    world.note_adventure(AdventureEvent::Iteration { n: 7 });
    world.note_adventure(AdventureEvent::Stall { level: 1 });
    world.note_adventure(AdventureEvent::Measured { improved: true });
    world.note_adventure(AdventureEvent::Measured { improved: false });
    world.note_adventure(AdventureEvent::Measured { improved: true });
    world
}

const SPEC_EXAMPLE: &str = "\u{2694} The Mines · iter 7 · \u{25A3}\u{25A3}\u{25A2} chests · danger \u{25AE}\u{25AF}\u{25AF} \
     · \u{2665}\u{2665}\u{2661} · party \u{2689}\u{2689}\u{2689} · \"treasure!\"";

#[test]
fn an_idle_town_keeps_todays_caption() {
    let (_lock, _comp, _turbo) = unlean();
    let world = World::new(3);
    assert_eq!(
        world.quest_hud(96),
        None,
        "Castle Town with no live banner must leave the caption alone"
    );
    // A banner raised in town (a retreat) is still worth a line.
    let mut retreated = spec_example_world();
    retreated.note_adventure(AdventureEvent::LoopFinished { ok: false });
    let line = retreated.quest_hud(96).expect("the retreat banner shows");
    let text = flat(&line);
    assert!(text.starts_with("\u{2654} Castle Town"), "{text:?}");
    assert!(text.ends_with("\"retreat\""), "{text:?}");
}

#[test]
fn the_widest_quest_line_is_the_documented_form() {
    let (_lock, _comp, _turbo) = unlean();
    let world = spec_example_world();
    let line = world.quest_hud(120).expect("an adventure is running");
    assert_eq!(flat(&line), SPEC_EXAMPLE);
    // Every glyph in the line is one display cell, so the composer's budget
    // arithmetic is the character count.
    assert_eq!(cell_width(SPEC_EXAMPLE), SPEC_EXAMPLE.chars().count());
}

#[test]
fn the_quest_line_drops_parts_right_to_left_and_never_overflows() {
    let (_lock, _comp, _turbo) = unlean();
    let world = spec_example_world();
    // 78 cells is the widest form; 77 loses the banner, and 40 is down to the
    // chests. The order is banner, party, hearts, danger, chests, iter.
    let at = |width: usize| flat(&world.quest_hud(width).expect("a live quest"));
    assert_eq!(cell_width(SPEC_EXAMPLE), 78);
    assert_eq!(at(78), SPEC_EXAMPLE);
    assert_eq!(
        at(77),
        "\u{2694} The Mines · iter 7 · \u{25A3}\u{25A3}\u{25A2} chests · danger \
         \u{25AE}\u{25AF}\u{25AF} · \u{2665}\u{2665}\u{2661} · party \u{2689}\u{2689}\u{2689}"
    );
    assert_eq!(
        at(40),
        "\u{2694} The Mines · iter 7 · \u{25A3}\u{25A3}\u{25A2} chests"
    );
    assert_eq!(at(11), "\u{2694} The Mines");
    assert_eq!(at(5), "\u{2694} Th\u{2026}");

    for width in 1..=90usize {
        let line = world.quest_hud(width).expect("a live quest");
        let text = flat(&line);
        assert!(
            cell_width(&text) <= width,
            "quest line overflowed {width} cells: {text:?}"
        );
    }
    assert_eq!(world.quest_hud(0), None, "a zero-cell row shows nothing");
}

#[test]
fn comp_mode_shows_none_of_the_quest_chrome() {
    let _lock = crate::tests::env_lock();
    let _comp = crate::tests::TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::comp_mode::invalidate_cache();
    let mut world = spec_example_world();
    assert_eq!(
        world.quest_hud(120),
        None,
        "lean mode keeps the route chrome"
    );
    world.note_adventure(AdventureEvent::Stall { level: 3 });
    assert_eq!(world.quest_border_style(), None);
    crate::comp_mode::invalidate_cache();
}

#[test]
fn danger_and_a_spent_budget_drain_the_hearts_without_punishing() {
    let (_lock, _comp, _turbo) = unlean();
    let mut world = spec_example_world();
    assert_eq!(world.quest_hearts(), (2, false));

    // The tightest of the three budget clocks decides: 9 of 10 iterations is
    // past the pressure line even though the tokens are barely touched.
    world.note_loop(
        true,
        7,
        false,
        false,
        Some(LoopBudgetSnapshot {
            iteration: 9,
            max_iters: 10,
            tokens_spent: 10,
            token_budget: 1_000_000,
            elapsed_secs: 0,
            deadline_secs: 0,
        }),
    );
    assert_eq!(world.quest_hearts(), (1, true));
    let line = world.quest_hud(120).expect("a live quest");
    let hearts = line
        .spans
        .iter()
        .find(|span| span.content.contains('\u{2665}'))
        .expect("a heart span");
    assert_eq!(hearts.content.as_ref(), "\u{2665}\u{2661}\u{2661}");
    assert_eq!(
        hearts.style.fg,
        Some(HUD_DIM),
        "pressure dims, never reddens"
    );

    // Deepest fog plus a spent budget still leaves the hero standing.
    world.note_adventure(AdventureEvent::Stall { level: 3 });
    assert_eq!(world.quest_hearts().0, 1);
    let text = flat(&world.quest_hud(120).expect("a live quest"));
    assert!(!text.contains("fail"), "no failure language: {text:?}");
}

#[test]
fn chests_cap_at_six_glyphs_and_then_carry_a_count() {
    let (_lock, _comp, _turbo) = unlean();
    let mut world = spec_example_world();
    for _ in 0..6 {
        world.note_adventure(AdventureEvent::Measured { improved: true });
    }
    let text = flat(&world.quest_hud(140).expect("a live quest"));
    assert!(
        text.contains("\u{25A3}\u{25A3}\u{25A3}\u{25A3}\u{25A3}\u{25A3} \u{00D7}9 chests"),
        "{text:?}"
    );
    // An empty chest is gold and hollow — never a warning colour.
    let chests = world
        .quest_hud(140)
        .expect("a live quest")
        .spans
        .iter()
        .find(|span| span.content.contains('\u{25A3}'))
        .map(|span| span.style.fg);
    assert_eq!(chests, Some(Some(HUD_GOLD)));
}

#[test]
fn the_pane_border_reads_the_quest() {
    let (_lock, _comp, _turbo) = unlean();
    // Calm: the shipped chrome.
    let mut world = spec_example_world();
    assert_eq!(world.quest_border_style(), None);

    // Deep fog: amber.
    world.note_adventure(AdventureEvent::Stall { level: 2 });
    assert_eq!(
        world.quest_border_style().and_then(|style| style.fg),
        Some(HUD_AMBER)
    );

    // A banner raised over the keep outranks the fog, for the banner's life.
    world.note_adventure(AdventureEvent::Submitted);
    world.note_adventure(AdventureEvent::SubmissionSettled { accepted: true });
    assert_eq!(
        world.quest_border_style().and_then(|style| style.fg),
        Some(HUD_GOLD)
    );
    // The world tick is the clock: 40 ticks later the banner is down and the
    // fog owns the border again.
    for _ in 0..41 {
        world.tick();
    }
    assert_eq!(
        world.quest_border_style().and_then(|style| style.fg),
        Some(HUD_AMBER),
        "the flash lasts exactly the banner's life"
    );
}

#[test]
fn the_region_prefixes_the_pane_title_and_the_status_strip() {
    let (_lock, _comp, _turbo) = unlean();
    let town = World::new(3);
    assert!(
        town.title().starts_with(" Realm · "),
        "town title unchanged: {:?}",
        town.title()
    );
    assert!(!town.title().contains("The Mines"));
    assert_eq!(town.quest_status_chrome(120), "");

    let world = spec_example_world();
    assert!(
        world.title().starts_with(" Realm · The Mines · "),
        "{:?}",
        world.title()
    );
    assert_eq!(world.quest_status_chrome(120), " · The Mines");
    assert_eq!(
        world.quest_status_chrome(24),
        "",
        "a narrow strip spends its cells on the live activity"
    );

    // The strip still fits every width it is handed with the region on it.
    for width in 0..=96usize {
        let status = world.status_line(width);
        assert!(
            super::super::spans_cell_width(&status.spans) <= width,
            "status overflowed {width} cells"
        );
    }
    let strip: String = world
        .status_line(120)
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect();
    assert!(strip.contains("· The Mines"), "{strip:?}");
}

#[test]
fn the_quest_report_names_every_field_the_renderers_read() {
    let world = spec_example_world();
    let report = world.quest_report();
    for needle in [
        "quest · The Mines",
        "competition",
        "iter 7",
        "danger 1/3",
        "treasure 2",
        "empty chests 1",
        "party 3",
        "waypoint ",
        "banner \"treasure!\"",
    ] {
        assert!(
            report.contains(needle),
            "{needle:?} missing from {report:?}"
        );
    }
    // Castle Town has no staged marks, so it reports no waypoint name.
    let town = World::new(3);
    let town_report = town.quest_report();
    assert!(
        town_report.contains("quest · Castle Town"),
        "{town_report:?}"
    );
    assert!(town_report.contains("waypoint 1/1"), "{town_report:?}");
}
