use super::*;

fn budget(tokens_spent: usize, token_budget: usize) -> LoopBudgetSnapshot {
    LoopBudgetSnapshot {
        iteration: 1,
        max_iters: 25,
        tokens_spent,
        token_budget,
        elapsed_secs: 10,
        deadline_secs: 3600,
    }
}

fn flat(text: &Text<'static>) -> String {
    text.lines
        .iter()
        .flat_map(|l| l.spans.iter())
        .map(|s| s.content.clone())
        .collect()
}

#[test]
fn world_generation_is_deterministic_and_playable() {
    let a = World::new(7);
    let b = World::new(7);
    assert_eq!(a.tiles, b.tiles);
    assert_eq!(a.buildings, b.buildings);
    assert_eq!(a.town_name, b.town_name);
    let c = World::new(8);
    assert_ne!(a.tiles, c.tiles, "different seeds → different islands");

    // Every building sits on the map with a distinct position.
    assert_eq!(a.buildings.len(), 8);
    for (i, &(_, p)) in a.buildings.iter().enumerate() {
        assert!(p.0 < WORLD_W && p.1 < WORLD_H);
        assert!(
            !a.buildings[i + 1..].iter().any(|&(_, q)| q == p),
            "buildings must not stack"
        );
    }
}

#[test]
fn route_is_walkable_and_deterministic() {
    let mut world = World::new(7);
    let start = world.avatar;
    world.target = Building::Observatory;
    let goal = world.dest();
    let first = world
        .least_cost_route(start, goal)
        .expect("roads connect the keep to the observatory");
    let second = world
        .least_cost_route(start, goal)
        .expect("the same route remains available");

    assert_eq!(first, second, "route selection must be deterministic");
    assert_eq!(first.last(), Some(&goal));
    let mut previous = start;
    for &tile in &first {
        assert_eq!(
            previous.0.abs_diff(tile.0) + previous.1.abs_diff(tile.1),
            1,
            "route step {previous:?} -> {tile:?} must be 4-adjacent"
        );
        previous = tile;
    }
}

#[test]
fn route_avoids_water_and_high_ground() {
    let mut world = World::new(7);
    world.target = Building::Observatory;
    let route = world
        .least_cost_route(world.avatar, world.dest())
        .expect("roads connect the keep to the observatory");
    let high_ground = route
        .iter()
        .filter(|&&(x, y)| matches!(world.at(x, y), Biome::Hill | Biome::Peak))
        .count();

    assert!(
        route
            .iter()
            .all(|&(x, y)| !matches!(world.at(x, y), Biome::Water | Biome::DeepWater))
    );
    assert_eq!(high_ground, 0, "seed 7's road route needs no high ground");
}

#[test]
fn knight_follows_route_to_arrival() {
    let mut world = World::new(7);
    world.target = Building::Observatory;
    let goal = world.dest();
    for _ in 0..2000 {
        if world.avatar == goal {
            break;
        }
        world.tick();
        let (x, y) = world.avatar;
        assert!(
            !matches!(
                world.at(x, y),
                Biome::Water | Biome::DeepWater | Biome::Hill | Biome::Peak
            ),
            "knight entered blocked or high-cost terrain at {:?}",
            world.avatar
        );
    }
    assert_eq!(world.avatar, goal, "knight must reach the observatory");
}

#[test]
fn retarget_mid_journey_recomputes() {
    let mut world = World::new(7);
    world.target = Building::Observatory;
    for _ in 0..12 {
        world.tick();
    }
    assert_ne!(world.avatar, world.dest(), "journey should still be active");

    world.target = Building::Gatehouse;
    let new_goal = world.dest();
    for _ in 0..2000 {
        if world.avatar == new_goal {
            break;
        }
        world.tick();
    }
    assert_eq!(world.avatar, new_goal, "knight must reach the new target");
    assert_eq!(world.route_goal, Some(new_goal));
}

#[test]
fn district_layout_is_seeded_sorted_and_deterministic() {
    let names = vec!["src", "tests", "docs", "scripts"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut a = World::new(77);
    let mut b = World::new(77);
    a.enable_districts(names.clone());
    b.enable_districts(names);
    assert_eq!(a.districts, b.districts);
    assert_eq!(
        a.districts
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        ["docs", "scripts", "src", "tests"]
    );
}

#[test]
fn empty_districts_preserve_wide_render_for_two_seeds() {
    for seed in [7, 42] {
        let untouched = World::new(seed);
        let mut enabled = World::new(seed);
        enabled.enable_districts(Vec::new());
        assert_eq!(
            fnv1a(flat(&untouched.render(80, 24)).as_bytes()),
            fnv1a(flat(&enabled.render(80, 24)).as_bytes())
        );
    }
}

#[test]
fn district_overflow_folds_into_outlands_and_empty_stays_empty() {
    let mut world = World::new(9);
    world.enable_districts(Vec::new());
    assert!(world.districts.is_empty());

    world.enable_districts((0..12).map(|i| format!("district-{i:02}")).collect());
    assert_eq!(world.districts.len(), 8);
    assert_eq!(
        world
            .districts
            .iter()
            .filter(|district| district.name == "Outlands")
            .count(),
        1
    );
}

#[test]
fn district_anchors_are_land() {
    let mut world = World::new(123);
    world.enable_districts(vec!["src".to_string(), "docs".to_string()]);
    for district in &world.districts {
        assert!(world.is_land(district.anchor.0 as usize, district.anchor.1 as usize));
    }
}

#[test]
fn tool_traffic_drives_the_avatar() {
    let mut w = World::new(42);
    w.note_tool_call("shell", "git push origin main");
    assert_eq!(w.target, Building::Gatehouse);
    assert!(w.carrying_mail, "a push carries the mail to town");

    w.note_tool_call("shell", "git commit -m 'x'");
    assert_eq!(w.target, Building::Rookery);
    w.note_tool_call("read_file", "cockpit/build.rs");
    assert_eq!(
        w.target,
        Building::Scriptorium,
        "reads win over fuzzy 'build'"
    );
    w.note_tool_call("shell", "cargo test");
    assert_eq!(w.target, Building::Smithy);
    w.note_tool_call("web_search", "ratatui scrollbar");
    assert_eq!(w.target, Building::Gatehouse);
    w.note_tool_call("delegate", "coder: look at this");
    assert_eq!(w.target, Building::RoundTable);
    w.note_tool_call("recall", "past decisions");
    assert_eq!(w.target, Building::Chapel);

    // Walking: ticks converge on the target and stop there. Pin the camera
    // wide so the chapel's close-range vignette (§6 M5) doesn't keep the
    // loop animating — this asserts the *walk* settled, not the camera.
    w.camera.mode = CameraMode::Wide;
    let target = w.building_pos(Building::Chapel);
    for _ in 0..(WORLD_W + WORLD_H) * 4 {
        w.tick();
    }
    assert_eq!(w.avatar, target);
    assert!(!w.animating());

    // Turn end sends the avatar home.
    w.turn_ended(true);
    assert_eq!(w.target, Building::Keep);
    assert!(w.animating());
}

#[test]
fn storms_and_errors_track_system_health() {
    let mut w = World::new(3);
    assert!(w.recent_errors < 3);
    w.note_notice("[club:longcat] quota exhausted — benched for 3600s");
    assert!(w.animating(), "storm keeps the animation ticking");
    assert_eq!(w.weather_label(), "storm");

    for _ in 0..3 {
        w.note_tool_call("shell", "cargo test");
        w.note_tool_result("shell", "error: it broke");
    }
    assert!(w.recent_errors >= 3, "errors pile up into rain");
    for _ in 0..4 {
        w.note_tool_call("shell", "cargo test");
        w.note_tool_result("shell", "ok: 812 passed");
    }
    assert!(w.recent_errors < 3, "passing calls clear the errors");

    // Compaction is a chapel vigil.
    w.note_notice("context compaction: folded 12 messages");
    assert_eq!(w.target, Building::Chapel);
}

#[test]
fn render_survives_any_pane_size() {
    let mut w = World::new(11);
    w.note_tool_call("shell", "git push");
    for _ in 0..30 {
        w.tick();
    }
    for (width, height) in [(0, 0), (1, 1), (3, 2), (20, 6), (46, 14), (200, 80)] {
        let text = w.render(width, height);
        if width > 0 && height > 1 {
            let flat: String = text
                .lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .map(|s| s.content.clone())
                .collect();
            let vw = (width as usize).min(WORLD_W);
            let vh = (height as usize).saturating_sub(1).clamp(1, WORLD_H);
            if vw >= 9 && vh >= 4 {
                assert!(
                    flat.chars().any(|c| ('\u{2801}'..='\u{28FF}').contains(&c)),
                    "Dotmax dots visible at {width}x{height}"
                );
                assert!(!flat.contains('@'), "no @ fallback at {width}x{height}");
            }
        }
    }
}

#[test]
fn the_quintain_absorbs_the_loop() {
    let mut w = World::new(7);
    assert!(
        !w.visiting_quintain(),
        "no quintain visit before a loop runs"
    );
    assert!(!w.loop_active);

    // A loop starts: the quintain turns.
    w.note_loop(true, 1, false, false, Some(budget(12_000, 2_000_000)));
    assert_eq!((w.loop_active, w.loop_iteration), (true, 1));

    // The resting knight rides to the quintain and settles — the cue to expand.
    for _ in 0..(WORLD_W + WORLD_H) * 2 {
        w.tick();
        if w.visiting_quintain() {
            break;
        }
    }
    assert!(
        w.visiting_quintain(),
        "knight settles at the quintain between rounds"
    );

    // Escalation storms the town on its rising edge.
    w.note_loop(true, 1, true, false, Some(budget(12_000, 2_000_000)));
    assert_eq!(w.weather_label(), "storm", "escalation storms the town");

    // The loop ends: the quintain stops and stops drawing the knight to it.
    w.note_loop(false, 0, false, true, None);
    assert!(!w.loop_active);
    assert!(!w.visiting_quintain());
}

#[test]
fn creatures_blink_and_talk() {
    fn flat(text: &Text<'static>) -> String {
        text.lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.clone())
            .collect()
    }
    let mut w = World::new(42);
    // Park the knight at the Round Table: council chat = a talking face. Pin
    // the camera wide so the close-range Round Table vignette (§6 M5) does
    // not keep `animating()` true — this asserts the walk settled.
    w.camera.mode = CameraMode::Wide;
    w.note_tool_call("delegate", "council");
    for _ in 0..(WORLD_W + WORLD_H) * 4 {
        w.tick();
    }
    assert!(!w.animating(), "settled at the Round Table");
    // Across a blink/talk window the dot field must visibly change even
    // though nothing is walking.
    let first = flat(&w.render(46, 14));
    let mut changed = false;
    for _ in 0..90 {
        w.tick();
        if flat(&w.render(46, 14)) != first {
            changed = true;
            break;
        }
    }
    assert!(
        changed,
        "face animation should alter the render across ticks"
    );
}

#[test]
fn town_name_outputs_match_pre_extraction_literals() {
    assert_eq!(town_name(7), "Corelot");
    assert_eq!(town_name(42), "Galilot");
    assert_eq!(town_name(123_456), "Camerfort");
}

#[test]
fn town_names_are_cute_and_stable() {
    assert_eq!(town_name(7), town_name(7));
    let n = town_name(123_456);
    assert!(n.chars().next().unwrap().is_uppercase());
    assert!(!n.contains('-'), "castle names carry no hyphen: {n}");
}

#[test]
fn road_band_is_a_centre_cross_not_a_slab() {
    // A path tile at close range is a half-tile cross through its centre —
    // the centre row/column are road, the corners are open ground.
    assert!(on_road_band(3.5, 7.5), "dead centre is road");
    assert!(on_road_band(3.1, 7.5), "vertical centre band is road");
    assert!(on_road_band(3.5, 7.1), "horizontal centre band is road");
    assert!(
        !on_road_band(3.1, 7.1),
        "a tile corner is NOT road (band, not slab)"
    );
    assert!(!on_road_band(3.9, 7.9), "opposite corner is NOT road");
}

fn flat_render(w: &World, width: u16, height: u16) -> String {
    w.render(width, height)
        .lines
        .iter()
        .flat_map(|l| l.spans.iter())
        .map(|s| s.content.clone())
        .collect()
}

#[test]
fn the_chapel_is_lit_only_while_memory_is_healthy() {
    use crate::knowledge::memory::store::MemoryHealth;
    for (health, lit) in [
        (MemoryHealth::Disabled, false),
        (MemoryHealth::Healthy, true),
        (MemoryHealth::Degraded, false),
    ] {
        let mut world = World::new(7);
        world.target = Building::Chapel;
        world.note_memory_health(health);
        assert_eq!(world.memory_health(), health);
        assert_eq!(world.overworld_scene().chapel_lit, lit, "{health:?}");
    }
}

#[test]
fn close_zoom_survives_size_and_zoom_matrix() {
    for (width, height) in [
        (0u16, 0u16),
        (1, 1),
        (3, 2),
        (20, 6),
        (46, 14),
        (80, 24),
        (200, 80),
    ] {
        for step in 0..=12u32 {
            let zoom = 1.0 + step as f32 * 0.25; // 1.0 ..= 4.0 in 0.25 steps
            let mut w = World::new(3);
            w.note_tool_call("shell", "git push");
            for _ in 0..20 {
                w.tick();
            }
            w.set_zoom_for_test(zoom);
            let _ = w.render(width, height); // must not panic or shear
        }
    }
}

#[test]
fn vignettes_do_not_spin_the_loop_when_wide() {
    // A smithy vignette rides the arrived state, but only animates at close
    // zoom — pinned wide, animating() still reaches false (§6 M5).
    let mut w = World::new(7);
    w.camera.mode = CameraMode::Wide;
    w.note_tool_call("write_file", "x"); // Smithy
    for _ in 0..600 {
        w.tick();
    }
    assert!(
        !w.animating(),
        "the wide view settles even at a vignette landmark"
    );
}

/// Drive the knight to its target and confirm arrival (bounded ticks).
fn walk_to_target(w: &mut World) {
    for _ in 0..(WORLD_W + WORLD_H) * 4 {
        if w.avatar == w.dest() {
            return;
        }
        w.tick();
    }
    assert_eq!(w.avatar, w.dest(), "knight should reach its target");
}

#[test]
fn auto_camera_punches_in_after_settling() {
    let mut w = World::new(7);
    w.note_tool_call("read_file", "src/main.rs"); // → Scriptorium
    walk_to_target(&mut w);
    // Not yet: a couple of settled ticks stays wide (hysteresis).
    w.tick();
    w.tick();
    assert_eq!(
        w.camera.zoom_target, Z_WIDE,
        "still wide before SETTLE_TICKS"
    );
    // Sit arrived past the settle threshold → punch in.
    for _ in 0..SETTLE_TICKS + 2 {
        w.tick();
    }
    assert_eq!(
        w.camera.zoom_target, Z_CLOSE,
        "auto camera punches in after settling"
    );
}

#[test]
fn new_target_pulls_the_camera_out_immediately() {
    let mut w = World::new(7);
    w.note_tool_call("read_file", "src/main.rs"); // Scriptorium
    walk_to_target(&mut w);
    for _ in 0..SETTLE_TICKS + 4 {
        w.tick();
    }
    assert_eq!(w.camera.zoom_target, Z_CLOSE, "settled close");
    // A new tool target reassigns dest(); the very next tick pulls out.
    w.note_tool_call("write_file", "src/lib.rs"); // Smithy (elsewhere)
    w.tick();
    assert_eq!(
        w.camera.zoom_target, Z_WIDE,
        "assigning a new target pulls the camera out at once"
    );
}

#[test]
fn easing_converges_so_the_loop_can_idle() {
    // MANDATORY (invariant 5): once motion + camera settle, animating() must
    // return false, or the event loop spins on the fast tick forever.
    let mut w = World::new(7);
    w.note_tool_call("read_file", "src/main.rs");
    for _ in 0..600 {
        w.tick();
    }
    assert!(
        !w.animating(),
        "camera + sub-tile motion settle so the loop stops ticking; \
             zoom={} target={} vis_settled={}",
        w.camera.zoom,
        w.camera.zoom_target,
        w.avatar_vis_settled()
    );
}

#[test]
fn small_pane_never_leaves_wide() {
    let mut w = World::new(7);
    w.note_tool_call("read_file", "src/main.rs");
    for _ in 0..600 {
        w.tick();
    }
    assert!(w.camera.zoom > 2.0, "camera punched in internally");
    // But a 20×6 pane pins wide (§5.1). Proof: the render ignores the close
    // zoom — it matches a render taken with the camera forced fully wide.
    let auto_small = flat_render(&w, 20, 6);
    w.camera.zoom = Z_WIDE;
    let wide_small = flat_render(&w, 20, 6);
    assert_eq!(
        auto_small, wide_small,
        "a 20×6 pane renders wide regardless of camera zoom"
    );
}

#[test]
fn world_zoom_cycles_modes() {
    let mut w = World::new(7);
    assert_eq!(w.camera.mode, CameraMode::Auto);
    assert_eq!(w.cycle_camera_zoom(), "wide");
    assert_eq!(w.camera.mode, CameraMode::Wide);
    assert_eq!(w.cycle_camera_zoom(), "close");
    assert_eq!(w.cycle_camera_zoom(), "auto");
    assert_eq!(w.camera.mode, CameraMode::Auto);
}

#[test]
fn avatar_vis_lags_then_settles() {
    let mut w = World::new(7);
    w.note_tool_call("read_file", "src/main.rs");
    let mut lagged = false;
    for _ in 0..(WORLD_W + WORLD_H) * 4 {
        w.tick();
        if !w.avatar_vis_settled() {
            lagged = true; // visual position trails the integer avatar
        }
        if w.avatar == w.dest() && w.avatar_vis_settled() {
            break;
        }
    }
    assert!(
        lagged,
        "avatar_vis eases behind the integer avatar mid-walk"
    );
    assert!(
        w.avatar_vis_settled(),
        "avatar_vis snaps to the avatar at rest"
    );
}

#[test]
fn centralized_classifier_uses_namespaced_leaves_and_bounded_shell_verbs() {
    let cases = [
        ("read_file", "", Building::Scriptorium, RealmActivity::Study),
        (
            "mcp__repo__symbols",
            "",
            Building::Scriptorium,
            RealmActivity::Study,
        ),
        ("write_file", "", Building::Smithy, RealmActivity::Forge),
        (
            "shell",
            "cargo test",
            Building::Smithy,
            RealmActivity::Forge,
        ),
        (
            "git_status",
            "",
            Building::Rookery,
            RealmActivity::Chronicle,
        ),
        (
            "shell",
            "git commit -m green",
            Building::Rookery,
            RealmActivity::Chronicle,
        ),
        ("git_push", "", Building::Gatehouse, RealmActivity::Dispatch),
        ("web__run", "", Building::Gatehouse, RealmActivity::Dispatch),
        ("delegate", "", Building::RoundTable, RealmActivity::Council),
        (
            "memory__recall",
            "",
            Building::Chapel,
            RealmActivity::Memory,
        ),
        (
            "science_search",
            "",
            Building::Observatory,
            RealmActivity::Research,
        ),
        ("novel_tool", "", Building::Keep, RealmActivity::Errand),
    ];
    for (name, args, building, activity) in cases {
        assert_eq!(
            classify_tool_activity(name, args),
            ClassifiedActivity { building, activity },
            "classification for {name} {args}"
        );
    }
    assert_ne!(
        classify_tool_activity("shell", "grep science.rs").building,
        Building::Observatory,
        "arbitrary shell prose cannot claim a discovery"
    );
}

#[test]
fn parallel_same_name_results_complete_their_own_landmark_records() {
    let mut w = World::new(17);
    let first = ToolEventId("read-a".to_string());
    let second = ToolEventId("read-b".to_string());
    w.note_tool_call_event(first.clone(), "read_file", "a.rs");
    w.note_tool_call_event(second.clone(), "read_file", "b.rs");

    w.note_tool_result_event(
        &second,
        "read_file",
        "b contents",
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::NotApplicable,
        },
    );
    assert!(w.active_work.contains_key(&first));
    assert!(!w.active_work.contains_key(&second));
    assert_eq!(w.recent_work.back().map(|work| &work.id), Some(&second));

    w.note_tool_result_event(
        &first,
        "read_file",
        "a contents",
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::NotApplicable,
        },
    );
    assert!(w.active_work.is_empty());
    assert_eq!(
        w.recent_work
            .iter()
            .map(|work| work.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["read-b", "read-a"]
    );
}

#[test]
fn invalid_and_inconclusive_outcomes_never_earn_or_sparkle() {
    let outcomes = [
        ToolOutcome {
            execution: ExecutionOutcome::NotStarted,
            verification: VerificationOutcome::NotApplicable,
        },
        ToolOutcome {
            execution: ExecutionOutcome::Failed,
            verification: VerificationOutcome::NotApplicable,
        },
        ToolOutcome {
            execution: ExecutionOutcome::Denied,
            verification: VerificationOutcome::NotApplicable,
        },
        ToolOutcome {
            execution: ExecutionOutcome::Cancelled,
            verification: VerificationOutcome::NotApplicable,
        },
        ToolOutcome {
            execution: ExecutionOutcome::Panicked,
            verification: VerificationOutcome::NotApplicable,
        },
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::Inconclusive,
        },
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::Failed,
        },
    ];
    for (index, outcome) in outcomes.into_iter().enumerate() {
        let mut w = World::new(index as u64 + 1);
        let id = ToolEventId(format!("bad-{index}"));
        w.note_tool_call_event(id.clone(), "science_search", "topic");
        w.note_tool_result_event(&id, "science_search", "not accepted", outcome);
        assert_eq!(w.renown, 0, "outcome {outcome:?}");
        assert_eq!(w.sparkle_until, 0, "outcome {outcome:?}");
    }
}

#[test]
fn not_started_tool_result_is_labeled_truthfully() {
    let mut w = World::new(41);
    let id = ToolEventId("guard-rejected".to_string());
    w.note_tool_call_event(id.clone(), "read_file", "path=src/lib.rs");
    w.note_tool_result_event(
        &id,
        "read_file",
        "tool error: reserved for verification",
        ToolOutcome {
            execution: ExecutionOutcome::NotStarted,
            verification: VerificationOutcome::NotApplicable,
        },
    );

    assert!(w.activity.contains("not started"), "{}", w.activity);
    assert_eq!(w.renown, 0);
    assert_eq!(w.sparkle_until, 0);
}

#[test]
fn unknown_and_duplicate_results_are_neutral_and_never_pay_twice() {
    let mut w = World::new(19);
    let known = ToolEventId("science-1".to_string());
    let unknown = ToolEventId("science-unknown".to_string());
    let success = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::NotApplicable,
    };
    w.note_tool_result_event(&unknown, "science_search", "ghost", success);
    assert_eq!((w.renown, w.event_diagnostics), (0, 1));
    w.note_tool_call_event(known.clone(), "science_search", "topic");
    w.note_tool_result_event(&known, "science_search", "discovery", success);
    assert_eq!(w.renown, 3);
    w.note_tool_result_event(&known, "science_search", "duplicate", success);
    assert_eq!((w.renown, w.event_diagnostics), (3, 2));
}

#[test]
fn teaching_journey_targets_the_scriptorium_without_faking_work_or_rewards() {
    let mut world = World::new(7);
    let starting_renown = world.renown();
    let starting_events = world.event_seq;
    assert!(world.active_work.is_empty());

    world.begin_teaching_lesson(&format!("{}overflow", "λ".repeat(80)));

    assert_eq!(world.target, Building::Scriptorium);
    assert_eq!(world.renown(), starting_renown);
    assert_eq!(world.event_seq, starting_events);
    assert!(world.active_work.is_empty());
    assert!(!world.carrying_mail);
    assert!(world.activity.starts_with("world lesson · "));
    assert!(world.activity.ends_with(" → Scriptorium"));
    assert!(
        world.activity.chars().count() < 90,
        "selected terms must stay bounded in world chrome"
    );
}

#[test]
fn ordinary_tool_activity_never_earns_progression() {
    let mut w = World::new(7);
    assert_eq!(w.level(), 0);
    assert_eq!(w.reward_chrome(), " Renown 0/10 · garden next");

    let call = ToolEventId("ordinary-build".to_string());
    w.note_tool_call_event(call.clone(), "shell", "cargo test");
    assert_eq!(w.renown, 0, "tool start is activity only");
    w.note_tool_result_event(
        &call,
        "shell",
        "148 passed",
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::NotApplicable,
        },
    );
    assert_eq!(w.renown, 0, "ordinary success has no payout");
    assert_eq!((w.xp, w.coins), (0, 0), "legacy fields remain unchanged");
}

#[test]
fn completed_turns_credit_one_renown_while_streak_stays_atmospheric() {
    let mut w = World::new(42);
    for expected in 1..=3u32 {
        w.turn_started();
        w.turn_ended(true);
        assert_eq!(w.streak, expected);
        assert_eq!(w.renown, u64::from(expected));
    }
    assert!(!w.reward_chrome().contains(" x3"));
    assert_eq!((w.xp, w.coins), (0, 0));

    w.turn_started();
    w.turn_ended(false);
    assert_eq!(w.streak, 0);
    assert_eq!(w.renown, 3, "a failed turn earns nothing");
}

/// A2: hidden-stage turn end clears work without renown/streak presentation tax.
#[test]
fn turn_ended_hidden_stage_clears_work_without_renown() {
    let mut w = World::new(7);
    w.turn_started();
    w.note_tool_call_event(
        crate::agent::harness::ToolEventId("t1".into()),
        "read_file",
        "path=src/lib.rs",
    );
    assert_eq!(w.active_work().count(), 1);
    let before = w.renown();
    w.turn_ended_hidden_stage(true);
    assert_eq!(w.renown(), before, "no renown credit when Stage hidden");
    assert_eq!(w.streak, 0, "no streak presentation when Stage hidden");
    assert_eq!(w.active_work().count(), 0, "in-flight work still cleared");
    w.turn_ended_hidden_stage(false);
    assert_eq!(
        w.fail_run, 1,
        "fail run still tracks for later visible Stage"
    );
}

#[test]
fn renown_thresholds_are_exact() {
    let thresholds = [10u64, 24, 42, 64, 96, 132, 172, 216];
    let mut w = World::new(5);
    for (index, threshold) in thresholds.into_iter().enumerate() {
        w.renown = threshold - 1;
        assert_eq!(w.level(), index as u32);
        w.renown = threshold;
        assert_eq!(w.level(), index as u32 + 1);
    }
    w.renown = 37;
    assert_eq!(w.reward_chrome(), " Renown 37/42 · lanterns next");
}

#[test]
fn each_turn_credits_at_most_one_verifier_and_one_research_bonus() {
    let mut w = World::new(23);
    w.turn_started();
    let verified = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::Passed,
    };
    for index in 0..2 {
        let id = ToolEventId(format!("verify-{index}"));
        w.note_tool_call_event(id.clone(), "shell", "cargo test");
        w.note_tool_result_event(&id, "shell", "148 passed", verified);
    }
    assert_eq!(w.renown, 4);
    assert_eq!(w.verified_wins, 1);

    let discovered = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::NotApplicable,
    };
    for index in 0..2 {
        let id = ToolEventId(format!("research-{index}"));
        w.note_tool_call_event(id.clone(), "science_search", "topic");
        w.note_tool_result_event(&id, "science_search", "discovery", discovered);
    }
    assert_eq!(w.renown, 7);
    w.turn_ended(true);
    assert_eq!(w.renown, 8);

    w.credit_renown("promotion:adapter-v2".to_string(), 10, "fleet promotion");
    w.credit_renown("promotion:adapter-v2".to_string(), 10, "fleet promotion");
    assert_eq!(w.renown, 18, "promotion celebration is event-deduplicated");
}

#[test]
fn working_on_science_files_is_not_a_discovery() {
    // The word "science" in tool ARGS (e.g. hacking on science.rs in this
    // very repo) must not route to the Observatory — only the tool NAME
    // carries that meaning. Otherwise every `grep … science.rs` success
    // would mint a false Discovery payout.
    let mut w = World::new(9);
    w.note_tool_call("shell", "grep -n reward src/science.rs");
    assert_ne!(w.target, Building::Observatory, "args alone must not route");
    let renown0 = w.renown;
    w.note_tool_result("shell", "ok");
    assert_eq!(w.renown, renown0, "no false research credit");
}

#[test]
fn a_science_result_at_the_observatory_mints_a_discovery() {
    let mut w = World::new(9);
    // A science tool routes the knight to the Observatory...
    w.note_tool_call("science_search", "graph neural networks");
    assert_eq!(w.target, Building::Observatory);
    // ...and its first successful result earns the one research award.
    w.note_tool_result("science_search", "◇ synthesis · 12 works");
    assert_eq!(w.renown, 3);
    assert_eq!((w.xp, w.coins), (0, 0));
}

fn finish_for_weather(world: &mut World, serial: usize, outcome: ToolOutcome) {
    let id = ToolEventId(format!("weather-{serial}"));
    world.note_tool_call_event(id.clone(), "shell", "weather truth");
    world.note_tool_result_event(&id, "shell", "weather truth", outcome);
}

#[test]
fn outcome_streak_forces_weather_and_verified_green_breaks_it() {
    let mut world = World::new(41);
    world.hearth.beats = (0..20_000)
        .find(|&beat| life::weather_for_beats(beat, world.seed) == life::Weather::Fair)
        .expect("seed must have fair weather");
    let red = ToolOutcome {
        execution: ExecutionOutcome::Failed,
        verification: VerificationOutcome::NotApplicable,
    };
    let green = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::Passed,
    };

    assert_eq!(world.outcome_weather(), life::Weather::Fair);
    finish_for_weather(&mut world, 0, red);
    assert_eq!(world.outcome_weather(), life::Weather::Fair);
    finish_for_weather(&mut world, 1, red);
    assert_eq!(world.outcome_weather(), life::Weather::Clouds);
    finish_for_weather(&mut world, 2, red);
    assert_eq!(world.outcome_weather(), life::Weather::Drizzle);
    finish_for_weather(&mut world, 3, green);
    assert_eq!(world.outcome_err_streak, 0);
    assert_eq!(world.outcome_weather(), life::Weather::Clearing);
}

#[test]
fn clearing_requires_green_after_red_lasts_fixed_beats_and_uses_fourth_bucket() {
    let mut world = World::new(43);
    let green = ToolOutcome {
        execution: ExecutionOutcome::Succeeded,
        verification: VerificationOutcome::Passed,
    };
    let red = ToolOutcome {
        execution: ExecutionOutcome::Panicked,
        verification: VerificationOutcome::NotApplicable,
    };
    finish_for_weather(&mut world, 0, green);
    assert_ne!(world.outcome_weather(), life::Weather::Clearing);
    finish_for_weather(&mut world, 1, red);
    finish_for_weather(&mut world, 2, green);
    assert_ne!(world.outcome_weather(), life::Weather::Clearing);
    finish_for_weather(&mut world, 3, red);
    finish_for_weather(&mut world, 4, red);
    let started = world.hearth.beats;
    finish_for_weather(&mut world, 5, green);
    assert_eq!(world.outcome_weather(), life::Weather::Clearing);
    assert_eq!((world.outcome_hearth_render_bucket() >> 8) & 3, 3);
    world.hearth.beats = started + life::CLEARING_BEATS - 1;
    assert_eq!(world.outcome_weather(), life::Weather::Clearing);
    world.hearth.beats = started + life::CLEARING_BEATS;
    assert_ne!(world.outcome_weather(), life::Weather::Clearing);
}

#[test]
fn weather_tracks_session_health() {
    let mut w = World::new(3);
    assert_eq!(w.weather_label(), "", "fair skies say nothing");

    // A gray run rains on the town; the first green after it hangs a
    // rainbow (and the rain dries out with it).
    w.turn_ended(false);
    assert!(!w.raining(), "one gray turn is not a run");
    w.turn_ended(false);
    assert!(w.raining());
    assert_eq!(w.weather_label(), "rain");
    w.turn_ended(true);
    assert_eq!(w.weather_label(), "rainbow");
    assert!(!w.raining(), "recovery dries the town out");

    // Clouds gather when the loop budget runs thin — and clear when the
    // loop puts the lance away.
    let mut w = World::new(3);
    w.note_loop(true, 1, false, false, Some(budget(80, 100)));
    assert_eq!(w.weather_label(), "clouds");
    w.note_loop(false, 0, false, true, None);
    assert_ne!(w.weather_label(), "clouds");

    // A quota storm outranks everything.
    w.note_notice("quota exhausted, failing over");
    w.turn_ended(false);
    w.turn_ended(false);
    assert_eq!(w.weather_label(), "storm");
}

#[test]
fn renown_v2_migrates_and_persists_without_losing_legacy_fields() {
    let path = std::env::temp_dir().join(format!(
        "angel-world-rewards-test-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    // A missing or corrupt file reads as a fresh start, never a crash.
    let fresh = World::load_rewards(&path);
    assert_eq!((fresh.renown, fresh.xp, fresh.coins), (0, 0, 0));
    std::fs::write(&path, "not json").unwrap();
    assert_eq!(World::load_rewards(&path).renown, 0);

    // Legacy XP/coin state migrates in memory and is not rewritten by read.
    std::fs::write(&path, r#"{"xp":140,"coins":9,"extra":"kept-on-read"}"#).unwrap();
    let legacy_before = std::fs::read_to_string(&path).unwrap();
    let migrated = World::load_rewards(&path);
    assert_eq!((migrated.renown, migrated.xp, migrated.coins), (23, 140, 9));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), legacy_before);

    // The next credited event atomically writes schema v2 and mirrors
    // renown into legacy prosperity while keeping rollback fields intact.
    let mut w = World::new(7);
    w.rewards_path = Some(path.clone());
    w.renown = migrated.renown;
    w.xp = migrated.xp;
    w.coins = migrated.coins;
    w.turn_started();
    w.turn_ended(true);
    let saved = World::load_rewards(&path);
    assert_eq!((saved.renown, saved.xp, saved.coins), (24, 140, 9));
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(raw["schema_version"], 2);
    assert_eq!(raw["hearth"]["prosperity"], 24);

    let _ = std::fs::remove_file(&path);
}

// ─── the village: the fleet mirrored into the town ──────────────────────

use crate::stage::village::{ForgeSnapshot, VillagePulse, VillageState};

const HEAD_IDS: &[&str] = &["dice", "math", "ocr"];

fn village_world(path: Option<std::path::PathBuf>) -> World {
    let mut w = World::new(7);
    let ids: Vec<String> = HEAD_IDS.iter().map(|s| s.to_string()).collect();
    w.enable_village(VillageState::default(), path, &ids);
    w
}

fn forge_pulse(f: ForgeSnapshot) -> VillagePulse {
    VillagePulse {
        forge: Some(f),
        ..Default::default()
    }
}

#[test]
fn the_forge_mirrors_training_and_promotions_celebrate_permanently() {
    let dir = std::env::temp_dir().join(format!("angel_wv_village_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("village.json");
    let mut w = village_world(Some(path.clone()));

    // Before any pulse the buildings are dark and the report says so.
    assert!(
        w.village_report().contains("DARK"),
        "{}",
        w.village_report()
    );
    assert!(!w.overworld_scene().forge_hot);

    // Training lights the smithy.
    w.note_village(forge_pulse(ForgeSnapshot {
        training: true,
        dataset_total: 10,
        ..Default::default()
    }));
    assert!(w.overworld_scene().forge_hot);
    assert!(w.village_report().contains("LIT"));

    // The adapter advancing = a promotion: fireworks, renown, and a
    // PERMANENT smithy upgrade persisted to disk.
    let renown_before = w.renown;
    w.note_village(forge_pulse(ForgeSnapshot {
        dataset_total: 12,
        adapter: Some("v1".into()),
        ..Default::default()
    }));
    assert!(w.tick < w.firework_until, "promotion fires the celebration");
    assert_eq!(w.renown, renown_before + 10, "promotion credits renown");
    assert!(w.activity.contains("promoted"), "{}", w.activity);
    let saved = crate::stage::village::load_state(Some(&path));
    assert_eq!(saved.last_adapter.as_deref(), Some("v1"));
    assert_eq!(saved.smithy_level, 1, "the upgrade is permanent");
    assert_eq!(saved.dataset_total, 12, "granary level persists");
    assert!(w.village_report().contains("adapter v1"));

    // The same adapter again is NOT a new celebration.
    for _ in 0..60 {
        w.tick();
    }
    w.note_village(forge_pulse(ForgeSnapshot {
        adapter: Some("v1".into()),
        ..Default::default()
    }));
    assert!(w.tick >= w.firework_until, "no re-celebration for old news");

    // Atlas going silent darkens the forge; the saved state is untouched.
    w.note_village(VillagePulse::default());
    assert!(w.village_report().contains("DARK"));
    assert_eq!(
        crate::stage::village::load_state(Some(&path)).smithy_level,
        1,
        "an outage never erases the village"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_gate_is_a_soot_puff_not_doom() {
    let mut w = village_world(None);
    let errors_before = w.recent_errors;
    w.note_village(forge_pulse(ForgeSnapshot {
        gate_pass: Some(false),
        ..Default::default()
    }));
    assert!(w.animating(), "the soot puff is a live (bounded) beat");
    assert_eq!(
        w.recent_errors, errors_before,
        "gentle: no error is counted"
    );
    assert_ne!(w.weather_label(), "storm", "no storm for a failed gate");
    // Bounded: the beat settles so the event loop can idle (invariant 5).
    for _ in 0..300 {
        w.tick();
    }
    assert!(!w.animating(), "the soot window closes");
    // The same failed gate reported again does not re-puff (edge, not level).
    let settled = w.tick;
    w.note_village(forge_pulse(ForgeSnapshot {
        gate_pass: Some(false),
        ..Default::default()
    }));
    assert!(
        w.village.as_ref().unwrap().soot_until <= settled,
        "soot fires on the falling edge only"
    );
}

#[test]
fn village_chatter_is_kept_briefly() {
    let mut w = village_world(None);
    w.note_village(VillagePulse {
        chatter: Some("the anvil remembers".into()),
        ..Default::default()
    });
    let village = w.village.as_ref().expect("village");
    assert_eq!(village.chatter_note, "the anvil remembers");
    assert!(w.animating(), "the chatter window keeps the frame alive");
}

#[test]
fn a_world_without_a_village_reports_and_renders_as_before() {
    let w = World::new(7);
    assert!(w.village_report().contains("no village"));
    // The wide fixtures above are the real guard; this is the cheap smoke:
    // no village glyphs appear on a bare world's map.
    let full = flat(&w.render(WORLD_W as u16, (WORLD_H + 1) as u16));
    assert!(!full.contains('⚅'), "no cottages without a village");
}

#[test]
fn muster_seat_states_recolor_the_ranks() {
    use crate::ui::viz::agentviz::SeatState;
    let mut w = World::new(7);
    w.form_muster("proposer wave 2", 4);
    assert_eq!(w.muster.len(), 4);
    let formed_glyph = w.muster[0].glyph;
    let formed_ink = w.muster[0].ink;

    w.apply_muster_states(&[
        SeatState::Returned,
        SeatState::Failed,
        SeatState::Cut,
        SeatState::Running,
    ]);
    assert_eq!(w.muster[0].glyph, '⚑', "a returned seat plants its banner");
    assert_eq!(
        w.muster[1].ink,
        DotColor::rgb(153, 27, 27),
        "failed dims red"
    );
    assert_eq!(
        w.muster[1].glyph, formed_glyph,
        "failure keeps the unit glyph"
    );
    assert_eq!(w.muster[2].ink, DotColor::rgb(87, 83, 78), "cut greys out");
    assert_eq!(
        w.muster[3].glyph, formed_glyph,
        "running seats are untouched"
    );
    assert_eq!(w.muster[3].ink, formed_ink);

    // An empty state vector (fresh publish) leaves the formation as formed,
    // and a longer-than-muster vector must not panic.
    w.form_muster("proposer wave 3", 2);
    w.apply_muster_states(&[]);
    assert_eq!(w.muster[0].glyph, formed_glyph);
    w.apply_muster_states(&[SeatState::Returned; 5]);
    assert_eq!(w.muster[1].glyph, '⚑');
}

#[test]
fn note_loop_adapter_emits_iteration_and_stall_into_the_quest() {
    let mut w = World::new(11);
    assert_eq!(w.quest().region(), Region::CastleTown);

    // The turn_io LoopMirror owns LoopStarted; direct note_loop callers still
    // reach the quest through the adapter's Iteration/Stall edges.
    w.note_adventure(AdventureEvent::LoopStarted {
        kind: LoopKind::Competition,
        task: "win the podrace".to_string(),
    });
    assert_eq!(w.quest().region(), Region::TheMines);
    assert_eq!(w.quest().kind(), LoopKind::Competition);
    w.note_loop(true, 1, false, false, Some(budget(1_000, 2_000_000)));
    assert_eq!(w.quest().iteration(), 1, "iteration edge reaches the quest");

    // Rising agitation is deep danger; falling clears the fog.
    w.note_loop(true, 2, true, false, Some(budget(1_000, 2_000_000)));
    assert_eq!(w.quest().danger().level(), 3);
    w.note_loop(true, 3, false, false, Some(budget(1_000, 2_000_000)));
    assert_eq!(w.quest().danger().level(), 0);

    assert_eq!(w.loop_iteration, 3);
    w.note_loop(false, 0, false, true, None);
    assert!(!w.loop_active);
}

#[test]
fn cinematic_key_tracks_the_quest_state() {
    let _view =
        crate::stage::world_viz::world3d::pin(crate::stage::world_viz::world3d::WorldView::Mesh3d);
    let start = |kind: LoopKind| AdventureEvent::LoopStarted {
        kind,
        task: String::new(),
    };
    let mut a = World::new(12);
    let mut b = World::new(12);
    assert_eq!(
        a.cinematic_key(),
        b.cinematic_key(),
        "identical worlds key equal"
    );

    // A region change re-keys the frame.
    a.note_adventure(start(LoopKind::Competition));
    assert_ne!(
        a.cinematic_key(),
        b.cinematic_key(),
        "region change re-keys"
    );
    b.note_adventure(start(LoopKind::Competition));
    assert_eq!(a.cinematic_key(), b.cinematic_key());

    // Loot re-keys too, and a different region differs from loot alone.
    a.note_adventure(AdventureEvent::Measured { improved: true });
    assert_ne!(a.cinematic_key(), b.cinematic_key(), "treasure re-keys");
    b.note_adventure(AdventureEvent::Stall { level: 2 });
    assert_ne!(a.cinematic_key(), b.cinematic_key());
    assert_eq!(b.quest().region(), Region::Swamp);
    assert_eq!(a.quest().region(), Region::TheMines);

    // Static room plates do not repaint for invisible outdoor quest changes.
    let mut c = World::new(12);
    let mut d = World::new(12);
    c.interior = Some(Building::Smithy);
    d.interior = Some(Building::Smithy);
    assert_eq!(c.cinematic_key(), d.cinematic_key());
    d.note_adventure(start(LoopKind::Research));
    assert_eq!(
        c.cinematic_key(),
        d.cinematic_key(),
        "room plates hold through invisible quest changes"
    );
}

#[test]
fn herald_names_the_place_the_deed_and_its_object() {
    let told = |name: &str, args: &str| herald(name, args).text(false);
    assert_eq!(
        told("read_file", "path=cockpit/src/ui/toolstrip.rs, offset=400"),
        "¶ Studying toolstrip.rs"
    );
    assert_eq!(
        told("grep", "pattern=note_row_text, path=cockpit"),
        "¶ Seeking \"note_row_text\""
    );
    assert_eq!(told("edit_file", "path=src/a.rs"), "⚒ Forging a.rs");
    assert_eq!(
        told("shell", "cd /repo && RUST_LOG=1 cargo test -p cockpit"),
        "⚔ Trial by cargo test"
    );
    assert_eq!(told("run_tests", ""), "⚔ Trial by the test suite");
    assert_eq!(told("shell", "git commit -m 'x'"), "✎ Sealing a commit");
    assert_eq!(
        herald("shell", "git commit -m 'x'").deed,
        activity::Deed::Seal
    );
    assert_eq!(told("shell", "git log --oneline"), "✎ Chronicling git log");
    assert_eq!(told("todo", "items=3"), "✎ Chronicling the todo");
    assert_eq!(
        told("web_fetch", "url=https://docs.rs/ratatui/latest"),
        "✉ Riding to docs.rs"
    );
    assert_eq!(told("wait_agent", "workers=a,b"), "⚜ Awaiting the council");
    assert_eq!(
        told("memory_recall", "query=bench pin"),
        "☩ Recalling \"bench pin\""
    );
    assert_eq!(
        told("arxiv", "query=sparse attention"),
        "☽ Consulting arxiv on \"sparse attention\""
    );
    assert_eq!(
        told("shell", "cmd=ls -la target, timeout=30"),
        "♜ Running ls"
    );
    assert_eq!(told("vision_look", "image"), "♜ Wielding vision look");
    assert_eq!(told("list_dir", "path=."), "¶ Studying the workspace");

    let forged = herald("write_file", "path=src/lib.rs");
    assert_eq!(forged.text(true), "⚒ Forged lib.rs");
    assert_eq!(&forged.text(true)[..forged.lead_len(true)], "⚒ Forged");
    let long = herald("read_file", &format!("path={}", "x".repeat(80)));
    assert_eq!(long.object.chars().count(), 40);
    assert!(long.object.ends_with('…'));
}
