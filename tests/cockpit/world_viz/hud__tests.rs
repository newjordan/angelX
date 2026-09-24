//! Z4 quest chrome: the pane border and the quest report.

use super::super::World;
use super::super::adventure::{AdventureEvent, LoopKind};
use crate::ui::hud::{HUD_AMBER, HUD_GOLD};

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
    crate::drive::comp_mode::invalidate_cache();
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

#[test]
fn comp_mode_shows_none_of_the_quest_chrome() {
    let _lock = crate::tests::env_lock();
    let _comp = crate::tests::TestEnvGuard::set("ANGEL_COMP_MODE", "1");
    crate::drive::comp_mode::invalidate_cache();
    let mut world = spec_example_world();
    world.note_adventure(AdventureEvent::Stall { level: 3 });
    assert_eq!(world.quest_border_style(), None);
    crate::drive::comp_mode::invalidate_cache();
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
