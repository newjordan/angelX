use super::*;

#[test]
fn world_tick_budget_preserves_wall_time_at_scenery_cadence() {
    assert_eq!(
        world_tick_budget(std::time::Duration::from_millis(24)),
        (0, false)
    );
    assert_eq!(
        world_tick_budget(std::time::Duration::from_millis(25)),
        (1, false)
    );
    assert_eq!(
        world_tick_budget(std::time::Duration::from_millis(83)),
        (3, false)
    );
    assert_eq!(
        world_tick_budget(std::time::Duration::from_millis(100)),
        (4, false)
    );
    assert_eq!(
        world_tick_budget(std::time::Duration::from_millis(200)),
        (8, false)
    );
}

#[test]
fn world_tick_budget_drops_pathological_backlog() {
    assert_eq!(
        world_tick_budget(std::time::Duration::from_millis(224)),
        (8, false)
    );
    assert_eq!(
        world_tick_budget(std::time::Duration::from_millis(225)),
        (8, true)
    );
    assert_eq!(
        world_tick_budget(std::time::Duration::from_secs(60)),
        (8, true)
    );
}

#[test]
fn the_route_in_hand_serves_from_its_model_familys_castle() {
    use crate::stage::houses;
    let mut app = App::new(
        crate::agent::club::Bag::for_render_test(&[
            ("local", &[("Qwen3.6-35B-A3B", true)]),
            ("kimi", &[("kimi-k3", true)]),
        ]),
        crate::ui::viewer::Viewer::new(),
    );
    app.bag.select_route(0, 0);
    // A self-hosted Qwen is a mode of `local`: the Qwen castle serves.
    assert_eq!(app.serving_houses().lead, houses::by_key("qwen"));
    app.bag.select_route(1, 0);
    let serving = app.serving_houses();
    assert_eq!(serving.lead, houses::by_key("kimi"));
    assert!(serving.seated.is_empty() && !serving.turn);
    app.advance();
    assert_eq!(
        app.world.overworld_scene().march.lead,
        houses::by_key("kimi")
    );
    assert_eq!(houses::serving().lead, houses::by_key("kimi"));
    houses::note_serving(&houses::Serving::default());
}
