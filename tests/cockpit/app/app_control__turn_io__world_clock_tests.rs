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
